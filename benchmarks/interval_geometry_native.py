"""Private native-Polars frontier competitor, including genuine lazy execution.

Only validation uses the shared native adapter. Sorting, prefix maxima, run
formation, canonical labels, grouping, clipping and output assembly are Polars
expressions. No callback collects a lazy query or iterates rows/groups in Python.
An error-only eager window translates grouped clustering diagnostics.
"""

import re

import polars as pl

from benchmarks.coverage_profile_native import _bound


def _prepare(intervals, start, end, by, domain=None, *, row_aligned=False):
    from polars_intervals import (
        _blocking_frame,
        _frame_dtypes,
        _frame_keys,
        _geometry_input,
        _target,
    )
    from polars_intervals._internal import validate_intervals

    if row_aligned:
        # Row-aligned windows inherit Polars grouping support, including endpoint
        # names and key dtypes that the geometry frame APIs deliberately reject.
        if not isinstance(intervals, (pl.DataFrame, pl.LazyFrame)):
            raise TypeError("intervals must be a DataFrame or LazyFrame")
        if not isinstance(start, str) or not isinstance(end, str):
            raise TypeError("start and end must be column names")
        keys = _frame_keys(by, ())
        selected = intervals.select(pl.selectors.by_name(list(dict.fromkeys([start, end, *keys]))))
        schema = selected.collect_schema()
        _frame_dtypes((schema[start], schema[end]), [], "native_cluster")
        dtype = schema[start]
    else:
        selected, keys, schema = _geometry_input(intervals, start, end, by, "native_geometry")
        dtype = schema["start"]
    bounds = None if domain is None else tuple(_bound(value, dtype) for value in domain)
    parsed = None if domain is None else tuple(_target(value) for value in domain)

    def validate(frame):
        try:
            validate_intervals(frame[start], frame[end], parsed)
        except pl.exceptions.ComputeError as error:
            # A frame validator reports a global row. Cluster windows instead
            # diagnose the original position within the affected group. Only
            # this exceptional path computes the corresponding native window.
            pattern = r"interval at index \d+ has start greater than end"
            if not (row_aligned and keys and re.search(pattern, str(error))):
                raise
            local = frame.select(
                pl.int_range(0, pl.len(), dtype=pl.UInt32)
                .over([pl.selectors.by_name(key) for key in keys])
                .filter(pl.selectors.by_name(start) > pl.selectors.by_name(end))
                .first()
            ).item()
            message = re.sub(
                pattern,
                f"interval at index {local} has start greater than end",
                str(error),
                count=1,
            )
            raise pl.exceptions.ComputeError(message) from error
        return frame

    # Shared barrier preserves all-row validation before downstream operations.
    source = _blocking_frame(selected, validate, selected.collect_schema()).lazy()
    source = source.select(
        pl.selectors.by_name(start).alias("_s"),
        pl.selectors.by_name(end).alias("_e"),
        *[pl.selectors.by_name(key).alias(f"_k{i}") for i, key in enumerate(keys)],
    )
    # Projecting into a fresh namespace makes temporary names collision-safe,
    # even if input columns are named _row, _g, *, or ^regex$.
    aliases = [f"_k{i}" for i in range(len(keys))]
    source = source.with_row_index("_row").with_columns(
        (pl.col("_row").min().over(aliases) if aliases else pl.lit(0, dtype=pl.UInt32)).alias("_g")
    )
    groups = (
        source.select("_g", *aliases).unique(maintain_order=True)
        if aliases
        else source.select((pl.len() * 0).alias("_g"))
    )
    return source, groups, keys, aliases, bounds


def _runs(source, include_touching):
    ordered = source.filter(pl.col("_s") < pl.col("_e")).sort("_g", "_s")
    ordered = ordered.with_columns(pl.col("_e").cum_max().shift().over("_g").alias("_prev"))
    boundary = (
        pl.col("_s") > pl.col("_prev") if include_touching else pl.col("_s") >= pl.col("_prev")
    )
    return ordered.with_columns(boundary.fill_null(True).alias("_new")).with_columns(
        pl.col("_new").cast(pl.UInt64).cum_sum().over("_g").alias("_run")
    )


def _finish(result, intervals):
    return result.collect() if isinstance(intervals, pl.DataFrame) else result


def native_cluster(intervals, *, start="start", end="end", by=None, include_touching=False):
    """Return one UInt32 cluster column aligned to original rows."""
    if type(include_touching) is not bool:
        raise TypeError("include_touching must be a Boolean")
    source, _, _, _, _ = _prepare(intervals, start, end, by, row_aligned=True)
    occupied = _runs(source, include_touching).select(
        "_g", "_row", pl.col("_row").min().over("_g", "_run").alias("_component")
    )
    empty = source.filter(pl.col("_s") == pl.col("_e")).select(
        "_g", "_row", pl.col("_row").alias("_component")
    )
    result = (
        pl.concat([occupied, empty])
        .sort("_row")
        .select(
            (pl.col("_component").rank("dense").over("_g").cast(pl.UInt64) - 1)
            .cast(pl.UInt32, strict=True)
            .alias("cluster")
        )
    )
    return _finish(result, intervals)


def _union(source):
    return (
        _runs(source, True)
        .group_by("_g", "_run")
        .agg(pl.col("_s").min(), pl.col("_e").max())
        .select("_g", "_s", "_e")
        .sort("_g", "_s")
    )


def _output(segments, groups, keys, aliases):
    return (
        segments.join(groups, on="_g")
        .sort("_g", "_s")
        .select(
            *[pl.col(alias).alias(key) for alias, key in zip(aliases, keys)],
            pl.col("_s").alias("start"),
            pl.col("_e").alias("end"),
        )
    )


def native_merge(intervals, *, start="start", end="end", by=None):
    """Canonical union with ordered observed groups and typed zero-row outputs."""
    source, groups, keys, aliases, _ = _prepare(intervals, start, end, by)
    return _finish(_output(_union(source), groups, keys, aliases), intervals)


def native_gaps(intervals, *, domain_start, domain_end, start="start", end="end", by=None):
    """Bounded complement, retaining groups whose coverage clips away."""
    source, groups, keys, aliases, bounds = _prepare(
        intervals, start, end, by, (domain_start, domain_end)
    )
    left, right = (pl.lit(bound).first() for bound in bounds)
    clipped = source.with_columns(
        pl.max_horizontal("_s", left).alias("_s"),
        pl.min_horizontal("_e", right).alias("_e"),
    )
    union = _union(clipped)
    leading = groups.join(union.group_by("_g").agg(pl.col("_s").min()), on="_g", how="left").select(
        "_g", left.alias("_s"), pl.col("_s").fill_null(right).alias("_e")
    )
    following = union.select(
        "_g",
        pl.col("_e").alias("_s"),
        pl.col("_s").shift(-1).over("_g").fill_null(right).alias("_e"),
    )
    gaps = pl.concat([leading, following]).filter(pl.col("_s") < pl.col("_e"))
    return _finish(_output(gaps, groups, keys, aliases), intervals)
