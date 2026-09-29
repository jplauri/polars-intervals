"""Private native-Polars competitor: complete validation and canonical output.

No Python row/group loops. Polars Int128 arithmetic is safe for public <=64-bit
quantities: the platform's maximum addressable row count times UInt64::MAX is
less than i128::MAX. This argument does not extend to wider input weights.
"""

from datetime import date, datetime

import polars as pl
from polars_intervals import _INTEGERS, _target


def _bound(value, dtype):
    parsed = _target(value)
    if dtype in _INTEGERS:
        if parsed["kind"] != "integer" or parsed.get("dtype", str(dtype)) != str(dtype):
            raise pl.exceptions.InvalidOperationError("domain dtype must match endpoints")
    elif dtype == pl.Date:
        if parsed["kind"] != "date":
            raise pl.exceptions.InvalidOperationError("domain dtype must match endpoints")
    elif parsed["kind"] != "datetime" or (parsed["unit"], parsed["timezone"]) != (
        dtype.time_unit,
        dtype.time_zone,
    ):
        raise pl.exceptions.InvalidOperationError("domain metadata must match endpoints")
    physical = (
        pl.Int32 if dtype == pl.Date else pl.Int64 if isinstance(dtype, pl.Datetime) else dtype
    )
    return pl.Series([int(parsed["value"])], dtype=physical, strict=True).cast(dtype)


def native_profile(
    intervals: pl.DataFrame,
    *,
    start: str = "start",
    end: str = "end",
    weight: str | None = None,
    by: str | list[str] | None = None,
    domain_start: int | date | datetime | pl.Series | None = None,
    domain_end: int | date | datetime | pl.Series | None = None,
    include_zero: bool = False,
    count_units: bool = False,
) -> pl.DataFrame:
    if not isinstance(intervals, pl.DataFrame):
        raise TypeError("collect explicitly; input must be an eager DataFrame")
    if (
        not isinstance(start, str)
        or not isinstance(end, str)
        or (weight is not None and not isinstance(weight, str))
    ):
        raise TypeError("column names must be strings")
    if type(include_zero) is not bool:
        raise TypeError("include_zero must be Boolean")
    keys = [] if by is None else [by] if isinstance(by, str) else by
    if not isinstance(keys, list) or any(not isinstance(k, str) for k in keys):
        raise TypeError("by must be a string or list of strings")
    if len(set(keys)) != len(keys) or set(keys) & {"start", "end", "load"}:
        raise ValueError("duplicate or reserved grouping keys")
    dtype = intervals[start].dtype
    if dtype != intervals[end].dtype or not (
        dtype in _INTEGERS or dtype == pl.Date or isinstance(dtype, pl.Datetime)
    ):
        raise pl.exceptions.InvalidOperationError("matching supported endpoint dtypes required")
    for k in keys:
        kd = intervals[k].dtype
        if not (
            kd in _INTEGERS or kd in (pl.String, pl.Boolean, pl.Date) or isinstance(kd, pl.Datetime)
        ):
            raise pl.exceptions.InvalidOperationError("unsupported group dtype")
    if weight is not None and intervals[weight].dtype not in _INTEGERS:
        raise pl.exceptions.InvalidOperationError("unsupported weight dtype")
    if (domain_start is None) != (domain_end is None):
        raise ValueError("both bounds must be supplied")
    bounds = (
        None if domain_start is None else (_bound(domain_start, dtype), _bound(domain_end, dtype))
    )
    if bounds and bounds[0].to_physical().item() > bounds[1].to_physical().item():
        raise pl.exceptions.ComputeError("reversed domain")
    # Validate original rows before filtering, with original row diagnostics.
    aliases = [f"_k{i}" for i in range(len(keys))]
    source = (
        intervals.lazy()
        .select(
            pl.col(start).alias("_s"),
            pl.col(end).alias("_e"),
            *[pl.col(k).alias(a) for k, a in zip(keys, aliases)],
            *([] if weight is None else [pl.col(weight).cast(pl.Int128).alias("_q")]),
        )
        .with_row_index("_row")
    )
    invalid = (
        source.select(
            (pl.col("_s").is_null() | pl.col("_e").is_null()).any().alias("null_end"),
            pl.col("_row").filter(pl.col("_s") > pl.col("_e")).min().alias("reversed"),
            *(
                []
                if weight is None
                else [
                    pl.col("_q").is_null().any().alias("null_weight"),
                    pl.col("_row").filter(pl.col("_q") < 0).min().alias("negative"),
                ]
            ),
        )
        .collect()
        .row(0, named=True)
    )
    if invalid["null_end"] or invalid.get("null_weight"):
        raise pl.exceptions.ComputeError("null endpoints or weights")
    for role in ("reversed", "negative"):
        if invalid.get(role) is not None:
            raise pl.exceptions.ComputeError(f"{role} at index {invalid[role]}")
    nonempty = pl.col("_s") < pl.col("_e")
    hull = [
        pl.col("_s").filter(nonempty).min().alias("_left"),
        pl.col("_e").filter(nonempty).max().alias("_right"),
    ]
    if aliases:
        groups = source.group_by(aliases, maintain_order=True).agg(
            pl.col("_row").min().alias("_g"), *hull
        )
    else:
        groups = source.select(pl.lit(0, dtype=pl.UInt32).alias("_g"), *hull)
    if bounds:
        groups = groups.with_columns(
            pl.lit(bounds[0]).first().alias("_left"), pl.lit(bounds[1]).first().alias("_right")
        )
    # Materialize only the group/horizon metadata, avoiding repeated group plans.
    groups = groups.collect().lazy()
    jobs = (
        source.join(groups, on=aliases, nulls_equal=True)
        if aliases
        else source.join(groups, how="cross")
    )
    jobs = jobs.with_columns(
        pl.when(pl.col("_s") < pl.col("_left"))
        .then(pl.col("_left"))
        .otherwise(pl.col("_s"))
        .alias("_s"),
        pl.when(pl.col("_e") > pl.col("_right"))
        .then(pl.col("_right"))
        .otherwise(pl.col("_e"))
        .alias("_e"),
    ).filter(pl.col("_s") < pl.col("_e"))
    if weight is not None:
        jobs = jobs.filter(pl.col("_q") > 0)
    if weight is None and count_units:
        starts = (
            jobs.group_by("_g", "_s")
            .agg(pl.len().cast(pl.Int128).alias("_delta"))
            .rename({"_s": "_x"})
        )
        ends = (
            jobs.group_by("_g", "_e")
            .agg((pl.lit(0, dtype=pl.Int128) - pl.len().cast(pl.Int128)).alias("_delta"))
            .rename({"_e": "_x"})
        )
    else:
        quantity = pl.lit(1, dtype=pl.Int128) if weight is None else pl.col("_q")
        starts = jobs.select("_g", pl.col("_s").alias("_x"), quantity.alias("_delta"))
        ends = jobs.select(
            "_g", pl.col("_e").alias("_x"), (pl.lit(0, dtype=pl.Int128) - quantity).alias("_delta")
        )
    edges = groups.filter(pl.col("_left") < pl.col("_right"))
    events = pl.concat(
        [
            starts.select("_g", "_x", "_delta"),
            ends.select("_g", "_x", "_delta"),
            edges.select(
                "_g", pl.col("_left").alias("_x"), pl.lit(0, dtype=pl.Int128).alias("_delta")
            ),
            edges.select(
                "_g", pl.col("_right").alias("_x"), pl.lit(0, dtype=pl.Int128).alias("_delta")
            ),
        ]
    )
    cells = (
        events.group_by("_g", "_x")
        .agg(pl.col("_delta").sum())
        .sort("_g", "_x")
        .with_columns(
            pl.col("_delta").cum_sum().over("_g").alias("load"),
            pl.col("_x").shift(-1).over("_g").alias("end"),
        )
        .rename({"_x": "start"})
        .filter(pl.col("start") < pl.col("end"))
    )
    if not include_zero:
        cells = cells.filter(pl.col("load") > 0)
    cells = cells.with_columns(
        (
            (pl.col("load") != pl.col("load").shift().over("_g"))
            | (pl.col("start") != pl.col("end").shift().over("_g"))
        )
        .fill_null(True)
        .alias("_change")
    ).with_columns(pl.col("_change").cast(pl.UInt64).cum_sum().over("_g").alias("_run"))
    segments = cells.group_by("_g", "_run", maintain_order=True).agg(
        pl.col("start").first(), pl.col("end").last(), pl.col("load").first()
    )
    return (
        segments.join(groups.select("_g", *aliases), on="_g")
        .sort("_g", "start")
        .select(*[pl.col(a).alias(k) for k, a in zip(keys, aliases)], "start", "end", "load")
        .collect()
    )
