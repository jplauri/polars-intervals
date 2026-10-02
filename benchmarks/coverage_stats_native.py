"""Private native-Polars competitor: grouped endpoint and query-boundary sweep.

All source records contribute separate start/end counters. Coverage integrates
the Boolean predicate ``starts > ends``, never its depth. Query boundaries
appear after source ends and before source starts at ties. Thus query-end
requests see starts strictly before their coordinate, while query-start
requests see ends at or before it. Payloads remain in a Polars Struct.
"""

import polars as pl
from polars_intervals import _blocking_frame, _coverage_stats_input, _internal


def native_coverage_stats(
    queries,
    intervals,
    *,
    query_start="start",
    query_end="end",
    interval_start="start",
    interval_end="end",
    by=None,
):
    """Return the complete public result, with shared native validation included."""
    eager = isinstance(queries, pl.DataFrame) and isinstance(intervals, pl.DataFrame)
    intervals, keys, query_schema = _coverage_stats_input(
        queries, intervals, query_start, query_end, interval_start, interval_end, by
    )
    aliases = [f"_k{i}" for i in range(len(keys))] or ["_k0"]
    payload_dtype = pl.Struct(query_schema)

    def branch(frame, start, end, source):
        return (
            frame.lazy()
            .select(
                pl.selectors.by_name(start).alias("_s"),
                pl.selectors.by_name(end).alias("_e"),
                *[pl.selectors.by_name(key).alias(alias) for key, alias in zip(keys, aliases)],
                *([] if keys else [pl.lit(0, dtype=pl.UInt8).alias("_k0")]),
                (
                    pl.lit(None, dtype=payload_dtype)
                    if source
                    else pl.struct(pl.selectors.by_name(*query_schema.names()))
                ).alias("_payload"),
            )
            .with_row_index("_row")
            .with_columns(pl.lit(source).alias("_source"))
        )

    combined = pl.concat(
        [
            branch(queries, query_start, query_end, False),
            branch(intervals, interval_start, interval_end, True),
        ],
        how="vertical",
    )

    def validate(frame):
        geometry = frame.select("_s", "_e", "_source", *aliases)
        query = geometry.filter(~pl.col("_source"))
        source = geometry.filter(pl.col("_source"))
        _internal.validate_coverage_stats(
            query["_s"],
            query["_e"],
            [query[key] for key in aliases] if keys else [],
            source["_s"],
            source["_e"],
            [source[key] for key in aliases] if keys else [],
        )
        return frame

    source = _blocking_frame(combined, validate, combined.collect_schema())
    at_start = pl.col("variable") == "_s"
    is_source = pl.col("_source")
    events = (
        source.filter(~is_source | (pl.col("_s") < pl.col("_e")))
        .with_columns(pl.col("_s", "_e").to_physical().cast(pl.Int128))
        .unpivot(
            on=["_s", "_e"],
            index=[*aliases, "_source", "_row", "_payload"],
            value_name="_x",
        )
        .with_columns(
            (is_source & at_start).cast(pl.UInt64).alias("_starts"),
            (is_source & ~at_start).cast(pl.UInt64).alias("_ends"),
            pl.when(~is_source).then(1).when(at_start).then(2).otherwise(0).alias("_tie"),
            pl.when(at_start).then(pl.col("_payload")).otherwise(None).alias("_payload"),
        )
        .group_by(*aliases, "_x", "_tie")
        .agg(
            pl.col("_starts", "_ends").sum(),
            pl.struct("_row", "_payload", "variable").filter(~is_source).alias("_requests"),
        )
        .sort(*aliases, "_x", "_tie")
        .with_columns(pl.col("_starts", "_ends").cum_sum().over(aliases))
        .with_columns(
            pl.when((pl.col("_starts") > pl.col("_ends")).shift().over(aliases))
            .then(pl.col("_x") - pl.col("_x").shift().over(aliases))
            .otherwise(pl.lit(0, dtype=pl.Int128))
            .alias("_covered")
        )
        .with_columns(pl.col("_covered").cum_sum().over(aliases))
    )
    result = (
        events.filter(pl.col("_tie") == 1)
        .explode("_requests", empty_as_null=False)
        .unnest("_requests")
        .group_by("_row")
        .agg(
            pl.col("_payload").drop_nulls().first(),
            (
                pl.col("_starts").filter(~at_start).first()
                - pl.col("_ends").filter(at_start).first()
            ).alias("overlap_count"),
            (pl.col("_covered").max() - pl.col("_covered").min()).alias("covered_length"),
            (pl.col("_x").max() - pl.col("_x").min()).alias("query_length"),
        )
        .with_columns(
            pl.when(pl.col("query_length") == 0)
            .then(pl.lit(0, dtype=pl.UInt64))
            .otherwise(pl.col("overlap_count"))
            .alias("overlap_count"),
            pl.when(pl.col("query_length") > 0)
            .then(
                pl.col("covered_length").cast(pl.Float64) / pl.col("query_length").cast(pl.Float64)
            )
            .otherwise(pl.lit(None, dtype=pl.Float64))
            .alias("covered_fraction"),
        )
        .sort("_row")
        .drop("_row")
        .unnest("_payload")
    )
    return result.collect() if eager else result
