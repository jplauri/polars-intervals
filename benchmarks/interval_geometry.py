"""Complete interval geometry calls, native plans, and separate RSS observations.

Run as ``uv run --no-sync python -m benchmarks.interval_geometry ...`` after
building and installing the release plugin. Workload construction, correctness
checks and returned-output destruction are outside every runtime sample.
"""

import polars as pl
import polars_intervals as pi

from benchmarks.coverage_profile import fixture as profile_fixture
from benchmarks.interval_geometry_native import native_cluster, native_gaps, native_merge
from benchmarks.provenance import polars_parser, run_polars

# family, order, dtype, requested groups (0=ungrouped), chunks, domain, payload, source
CASES = [
    ("sequencing", "start", "i64", 0, 1, "extended", 0, "memory"),
    ("sequencing", "shuffled", "i64", 0, 1, "extended", 0, "memory"),
    ("nested", "start", "i64", 0, 1, "partial", 0, "memory"),
    ("disjoint", "shuffled", "u64", 0, 1, "extended", 0, "memory"),
    ("touching", "reverse", "i64", 0, 1, "extended", 0, "memory"),
    ("spanning", "partial", "i64", 0, 7, "extended", 0, "memory"),
    ("repeated", "shuffled", "date", 0, 1, "partial", 0, "memory"),
    ("sequencing", "shuffled", "datetime_ns", 1000, 7, "extended", 0, "memory"),
    ("booking", "shuffled", "i64", 8, 7, "partial", 0, "memory"),
    ("empty", "start", "i64", 32, 1, "extended", 0, "memory"),
    ("booking", "shuffled", "i64", 32, 1, "outside", 0, "memory"),
    ("sparse", "partial", "u64", 0, 1, "extended", 0, "memory"),
    ("booking", "shuffled", "i64", 32, 7, "partial", 8, "memory"),
    ("sequencing", "shuffled", "i64", 8, 7, "extended", 0, "parquet"),
    ("booking_skew", "shuffled", "i64", 32, 7, "partial", 0, "memory"),
]
DIMENSIONS = ["family", "order", "dtype", "groups", "chunks", "domain", "payload", "source"]
OPERATIONS = ("cluster_strict", "cluster_touching", "merge", "gaps")
SCOPES = ("eager", "lazy_complete", "lazy_plan", "lazy_collect", "streaming_complete")


def fixture(n, case, seed):
    family, order, dtype, groups, chunks, domain, payload, _ = case
    underlying = (
        "disjoint" if family == "spanning" else "booking" if family == "booking_skew" else family
    )
    df, options = profile_fixture(
        n, (underlying, order, dtype, groups, chunks, "units", domain, False), seed
    )
    if family == "spanning" and n:
        # One huge interval and many short/empty rows, with unsorted end points.
        df = (
            df.with_row_index("row")
            .with_columns(
                pl.when(pl.col("row") == 0).then(10).otherwise(pl.col("start")).alias("start"),
                pl.when(pl.col("row") == 0)
                .then(3 * n + 11)
                .when(pl.col("row") % 7 == 0)
                .then(pl.col("start"))
                .otherwise(pl.col("end"))
                .alias("end"),
            )
            .drop("row")
        )
    if payload:
        df = df.with_columns(
            *[
                pl.concat_str(
                    pl.lit("irrelevant metadata " * 12),
                    pl.int_range(pl.len()).cast(pl.String),
                    pl.lit(str(i)),
                ).alias(f"payload{i}")
                for i in range(payload)
            ]
        )
    if family == "booking_skew":
        df = df.with_columns(
            pl.when(pl.int_range(pl.len()) < n * 3 // 4)
            .then(None)
            .otherwise(pl.col("group"))
            .alias("group")
        )
    return df, {key: options[key] for key in ("by", "domain_start", "domain_end")}


def solve(source, operation, method, options):
    by = options["by"]
    if operation.startswith("cluster"):
        touching = operation == "cluster_touching"
        if method == "native":
            return native_cluster(source, by=by, include_touching=touching)
        expr = pi.cluster_intervals("start", "end", include_touching=touching)
        if by:
            expr = expr.over(by)
        return source.select(expr.alias("cluster"))
    if operation == "merge":
        return (native_merge if method == "native" else pi.merge_intervals)(source, by=by)
    return (native_gaps if method == "native" else pi.interval_gaps)(source, **options)


def statistics(df, options, operation, result):
    nonempty = pl.col("start") < pl.col("end")
    clipped = (
        nonempty
        & (pl.col("start") < pl.lit(options["domain_end"]).first())
        & (pl.col("end") > pl.lit(options["domain_start"]).first())
    )
    m, clipped_n = df.select(nonempty.sum().alias("m"), clipped.sum().alias("clipped")).row(0)
    group_sizes = df.group_by("group").len()["len"] if options["by"] else pl.Series([df.height])
    components = result["cluster"].n_unique() if operation.startswith("cluster") else result.height
    if options["by"] and operation.startswith("cluster"):
        components = df.select("group").hstack(result).n_unique()
    return {
        "nonempty": m,
        "clipped": clipped_n,
        "output_rows": result.height,
        "components_or_segments": components,
        "observed_groups": len(group_sizes),
        "largest_group": group_sizes.max() or 0,
        "column_chunks": "|".join(str(column.n_chunks()) for column in df),
        "domain_left": options["domain_start"].to_physical().item(),
        "domain_right": options["domain_end"].to_physical().item(),
    }


def memory_fields(_frames, before, after, result):
    return {
        "before_rss_bytes": before["rss_bytes"],
        "before_peak_bytes": before["peak_rss_bytes"],
        "after_rss_bytes": after["rss_bytes"],
        "after_peak_bytes": after["peak_rss_bytes"],
        "output_rows": result.height,
    }


def main():
    parser = polars_parser(__doc__, CASES, [0, 8, 1000, 10000, 100000], SCOPES, SCOPES)
    parser.add_argument("--operations", nargs="+", choices=OPERATIONS, default=list(OPERATIONS))
    args = parser.parse_args()
    notes = {
        "scope": "Complete validation/preparation/grouping/clipping/sorting/scan/remapping/output/FFI. Eager and lazy_complete include plan construction. lazy_plan does not execute. lazy_collect collects a prebuilt plan. streaming_complete builds and collects using the streaming engine. Returned output destruction, fixture construction, correctness and RSS calls are excluded.",
        "memory": "Separate fresh-process eager call. Before/after process working set and peak working set on Windows (RSS/high-water mark elsewhere), includes inputs, runtime and retained output. Peak increase after fixture construction is not exact allocations and may be zero if an earlier high-water mark dominates. No timed RSS polling.",
        "validation": "Every sampled result checked against actual production outside timing. Independent small graph and elementary-cell candidate oracles: benchmarks/test_interval_geometry.py. Large cases use cross-implementation equality, not an independent large oracle.",
        "native_candidate": "Native Polars all-row validation, called directly for eager inputs and behind a blocking no-pushdown map_batches barrier for lazy inputs. Geometry also uses native lazy expressions. Cum-max includes ALL preceding ends. Empty singletons and first-occurrence remapping included.",
        "ordering": "Candidate order rotates per sample. Both methods warmed before samples. Plan-only output schema checked outside timing.",
        "omissions": "No profile baseline: optional coverage-profile comparison omitted to keep the focused A/B/C experiment. Scan-backed cases have no eager scope. Input-generation integer arithmetic is fixture-only. No out-of-core claim.",
    }
    run_polars(
        "interval_geometry",
        parser,
        args,
        CASES,
        DIMENSIONS,
        fixture,
        solve,
        statistics,
        notes,
        operations=args.operations,
        memory_fields=memory_fields,
        omit=("memory_sha256",),
    )


if __name__ == "__main__":
    main()
