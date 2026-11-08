"""Complete per-query coverage calls versus a native Polars boundary sweep.

Run with ``uv run --no-sync python -m benchmarks.coverage_stats --output ...``
after rebuilding the release plugin. ``size`` is max(n,m), not their sum.
Every raw sample records query n and source m separately.
"""

import polars as pl
import polars_intervals as pi

from benchmarks.coverage_stats_native import native_coverage_stats
from benchmarks.provenance import polars_parser, run_polars

DIMENSIONS = [
    "family",
    "query_order",
    "interval_order",
    "ratio",
    "dtype",
    "groups",
    "matching",
    "chunks",
    "query_payload",
    "source_payload",
    "source",
]
CASES = [
    ("genomic", "start", "start", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("genomic", "shuffled", "reverse", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("outages", "partial", "shuffled", "equal", "datetime_ns", 32, "all", 7, 0, 0, "memory"),
    ("nested", "start", "shuffled", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("duplicates", "shuffled", "reverse", "equal", "date", 0, "all", 1, 0, 0, "memory"),
    ("touching", "reverse", "start", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("no_hit", "shuffled", "shuffled", "equal", "u64", 0, "all", 1, 0, 0, "memory"),
    ("all_covered", "shuffled", "reverse", "few_queries", "i64", 0, "all", 7, 0, 0, "memory"),
    ("all_covered", "start", "shuffled", "few_sources", "i64", 0, "all", 7, 0, 0, "memory"),
    ("empty_sources", "shuffled", "start", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("genomic", "shuffled", "reverse", "equal", "datetime_ns", 1000, "all", 7, 0, 0, "memory"),
    ("genomic", "partial", "shuffled", "equal", "i64", 32, "sparse", 7, 8, 8, "memory"),
    ("genomic", "shuffled", "shuffled", "equal", "i64", 8, "all", 7, 0, 0, "parquet"),
    ("skew", "shuffled", "reverse", "equal", "i64", 32, "all", 7, 0, 0, "memory"),
    ("sparse", "partial", "reverse", "equal", "u64", 0, "all", 1, 0, 0, "memory"),
    ("empty_queries", "start", "shuffled", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("empty_heavy", "start", "shuffled", "equal", "i64", 32, "all", 7, 0, 0, "memory"),
    ("disjoint", "shuffled", "shuffled", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("all_covered", "start", "reverse", "equal", "i64", 0, "all", 1, 0, 0, "memory"),
    ("genomic", "shuffled", "shuffled", "equal", "i64", 0, "all", 7, 16, 16, "memory"),
    ("genomic", "shuffled", "shuffled", "equal", "i64", 0, "all", 7, 0, 0, "memory"),
    ("genomic", "shuffled", "shuffled", "equal", "i64", 0, "all", 7, 0, 16, "memory"),
]
SCOPES = (
    "eager",
    "lazy_complete",
    "lazy_plan",
    "lazy_collect",
    "mixed_complete",
    "streaming_complete",
)


def fixture(size, case, seed):
    (
        family,
        query_order,
        interval_order,
        ratio,
        dtype,
        groups,
        matching,
        chunks,
        query_payload,
        source_payload,
        _,
    ) = case
    n, m = (
        (min(8, size) if ratio == "few_queries" else size),
        (min(8, size) if ratio == "few_sources" else size),
    )
    n = 0 if family == "empty_queries" else n
    m = 0 if family == "empty_sources" else m
    endpoint_dtype = {
        "i64": pl.Int64,
        "u64": pl.UInt64,
        "date": pl.Date,
        "datetime_ns": pl.Datetime("ns", "UTC"),
    }[dtype]

    def side(count, source, order, payload):
        df = pl.DataFrame({"i": pl.int_range(count, eager=True, dtype=pl.Int64)})
        i = pl.col("i")
        start = 10 * i + (2 if source else 0)
        end = start + (8 if source else 12)
        if family in ("genomic", "skew"):
            start = i * (7 if source else 11)
            end = start + ((i * 37 + seed % 101) % 101) + (1 if source else 20)
        elif family == "outages":
            start = i * (13 if source else 10)
            end = start + (i % 9 + 1 if source else 10)
        elif family == "nested":
            start, end = i, 2 * size - i
        elif family == "duplicates":
            start, end = i % 8, i % 8 + 12
        elif family == "touching":
            start, end = i * 4 + (2 if source else 0), i * 4 + (4 if source else 2)
        elif family == "no_hit":
            start, end = i * 10 + (5 if source else 0), i * 10 + (8 if source else 3)
        elif family == "all_covered":
            start, end = (pl.lit(0), pl.lit(4 * size + 1)) if source else (i, i + size + 1)
        elif family == "disjoint":
            start, end = i * 10, i * 10 + (4 if source else 8)
        elif family == "sparse":
            start = i * 10_000_000_000 + (2 if source else 0)
            end = start + (4 if source else 8)
        elif family == "empty_heavy":
            end = pl.when(i % 5 != 0).then(start).otherwise(end)
        if dtype == "u64":
            start, end = start.cast(pl.UInt64) + 2**63, end.cast(pl.UInt64) + 2**63
        df = df.with_columns(
            start.cast(endpoint_dtype).alias("start"), end.cast(endpoint_dtype).alias("end")
        )
        if groups:
            group = i % groups + (groups - 1 if matching == "sparse" and source else 0)
            group = pl.when(i % 19 == 0).then(None).otherwise(group)
            if family == "skew":
                group = pl.when(i < count * 3 // 4).then(None).otherwise(group)
            df = df.with_columns(group.cast(pl.Int64).alias("group"))
        if order == "reverse":
            df = df.reverse()
        elif order == "shuffled":
            df = df.sample(fraction=1, shuffle=True, seed=(seed + source) % 2**64)
        elif order == "partial":
            df = df.sort(pl.col("i") // 64, -(pl.col("i") % 64))
        if payload:
            df = df.with_columns(
                *[
                    pl.concat_str(pl.col("i"), pl.lit(f" payload{j} " * 20)).alias(f"payload{j}")
                    for j in range(payload)
                ],
                pl.concat_list("i", "start").alias("nested_list"),
                pl.struct("i", "end").alias("nested_struct"),
            )
        df = df.drop("i")
        if chunks > 1 and count:
            step = max(1, count // chunks)
            df = pl.concat(
                [df.slice(offset, step) for offset in range(0, count, step)], rechunk=False
            )
        return df

    return (
        side(n, False, query_order, query_payload),
        side(m, True, interval_order, source_payload),
        {"by": "group" if groups else None},
    )


def solve(queries, intervals, _operation, method, options):
    return (pi.coverage_stats if method == "production" else native_coverage_stats)(
        queries, intervals, **options
    )


def statistics(queries, intervals, options, _operation, _expected):
    by = options["by"]
    sizes = queries.group_by(by).len()["len"] if by else pl.Series([queries.height])
    matched = (
        queries.select(by)
        .unique()
        .join(intervals.select(by).unique(), on=by, nulls_equal=True)
        .height
        if by
        else int(bool(queries.height and intervals.height))
    )
    lengths = queries.select(
        (
            pl.col("end").to_physical().cast(pl.Int128)
            - pl.col("start").to_physical().cast(pl.Int128)
        ).alias("length")
    )["length"]
    return {
        "n": queries.height,
        "m": intervals.height,
        "source_nonempty": intervals.select((pl.col("start") < pl.col("end")).sum()).item(),
        "source_distinct_rows": intervals.select(*([by] if by else []), "start", "end")
        .unique()
        .height,
        "p": pi.merge_intervals(intervals, **options).height,
        "distinct_endpoints": pl.concat(
            [frame[name] for frame in (queries, intervals) for name in ("start", "end")]
        ).n_unique(),
        "query_span_min": lengths.min() or 0,
        "query_span_median": lengths.median() or 0,
        "query_span_max": lengths.max() or 0,
        "query_groups": len(sizes) if by else int(bool(queries.height)),
        "matched_groups": matched,
        "largest_query_group": sizes.max() or 0,
        "query_chunks": queries["start"].n_chunks(),
        "interval_chunks": intervals["start"].n_chunks(),
    }


def memory_fields(frames, before, after, result):
    return {
        "n": frames[0].height,
        "m": frames[1].height,
        "before": before,
        "after": after,
        "output_rows": result.height,
    }


def main():
    parser = polars_parser(
        __doc__, CASES, [8, 1000, 10000, 100000], SCOPES, ("eager", "lazy_complete")
    )
    notes = {
        "size": "size=max(n,m), with query n and source m recorded separately in every sample",
        "scope": "Both-side validation, grouping, source preparation, queries, four statistics, payload preservation and output materialization. Eager/lazy_complete/mixed_complete/streaming_complete include planning. lazy_plan excludes execution; lazy_collect uses a prebuilt plan. Input construction, correctness and returned-output destruction excluded.",
        "memory": "Separate cold process eager call: process RSS/peak RSS including runtime, inputs and retained output. Peak increase after fixture construction is not an allocation count or algorithm heap metric.",
        "verification": "Independent original-row elementary-cell oracle for all small fixtures in benchmarks/test_coverage_stats.py. Every timed complete result compared against production outside timing. Large comparisons are candidate agreement, not an independent oracle.",
        "native_candidate": "Grouped native endpoint/query-boundary sort, separate cumulative record starts/ends and exact Int128 integral of Boolean occupancy. All-row Polars validation included behind a whole-combined-input blocking boundary. Query payload carried in a Struct. No Python row/group loops, overlap pairs, or nested collect.",
        "ordering": "Warmups then rotating method order for each sample. Both return complete materialized four-statistic outputs.",
        "limitations": "Synthetic single-machine data. Scan cases include warm local Parquet I/O and are separate from memory cases. No out-of-core claim or hardware counters.",
    }
    run_polars(
        "coverage_stats",
        parser,
        parser.parse_args(),
        CASES,
        DIMENSIONS,
        fixture,
        solve,
        statistics,
        notes,
        size_column="size",
        memory_fields=memory_fields,
        record_cpu=True,
    )


if __name__ == "__main__":
    main()
