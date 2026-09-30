"""Complete interval geometry calls, native plans, and separate RSS observations.

Run as ``uv run --no-sync python -m benchmarks.interval_geometry ...`` after
building and installing the release plugin. Workload construction, correctness
checks and returned-output destruction are outside every runtime sample.
"""

import argparse
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars.testing import assert_frame_equal
from polars_intervals import _internal

from benchmarks.coverage_profile import fixture as profile_fixture
from benchmarks.interval_geometry_native import native_cluster, native_gaps, native_merge
from benchmarks.provenance import (
    ROOT,
    archive_sources,
    build_environment,
    environment,
    resident_memory,
    sha256,
    source_changes,
    verify_release,
)

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


def memory_worker(values):
    case_id, n, seed, operation, method = values
    df, options = fixture(int(n), CASES[int(case_id)], int(seed))
    before = resident_memory()
    result = solve(df, operation, method, options)
    after = resident_memory()
    print(
        json.dumps(
            {
                "case": int(case_id),
                "n": int(n),
                "seed": int(seed),
                "operation": operation,
                "method": method,
                "before_rss_bytes": before["rss_bytes"],
                "before_peak_bytes": before["peak_rss_bytes"],
                "after_rss_bytes": after["rss_bytes"],
                "after_peak_bytes": after["peak_rss_bytes"],
                "peak_increase_bytes": max(0, after["peak_rss_bytes"] - before["peak_rss_bytes"]),
                "output_rows": result.height,
            }
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--sizes", nargs="+", type=int, default=[0, 8, 1000, 10000, 100000])
    parser.add_argument("--seeds", nargs="+", type=int, default=[7, 41])
    parser.add_argument(
        "--cases", nargs="+", type=int, choices=range(len(CASES)), default=list(range(len(CASES)))
    )
    parser.add_argument("--operations", nargs="+", choices=OPERATIONS, default=list(OPERATIONS))
    parser.add_argument("--scopes", nargs="+", choices=SCOPES, default=list(SCOPES))
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument(
        "--memory", action="store_true", help="Separate fresh-process eager working-set samples"
    )
    parser.add_argument("--memory-worker", nargs=5, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.memory_worker:
        memory_worker(args.memory_worker)
        return
    if args.output is None or min(args.sizes) < 0 or args.samples < 1 or args.warmups < 1:
        parser.error("Need --output, nonnegative sizes and positive samples/warmups")
    native = Path(_internal.__file__)
    try:
        release, inputs = verify_release(native)
    except ValueError as error:
        parser.error(str(error))
    raw = args.output.with_suffix(".csv")
    meta = args.output.with_suffix(".metadata.json")
    archive = args.output.with_suffix(".sources.zip")
    memory = args.output.with_suffix(".memory.json")
    if any(path.exists() for path in (raw, meta, archive, memory)):
        parser.error("Use a new output prefix")
    raw.parent.mkdir(parents=True, exist_ok=True)
    sources = list(
        dict.fromkeys(
            [
                *inputs,
                *ROOT.glob("python/polars_intervals/*.py"),
                *ROOT.glob("benchmarks/interval_geometry*.py"),
                ROOT / "benchmarks/test_interval_geometry.py",
                ROOT / "tests/test_interval_geometry.py",
                ROOT / "tests/dtypes.py",
                ROOT / "benchmarks/coverage_profile.py",
                ROOT / "benchmarks/coverage_profile_native.py",
                ROOT / "benchmarks/provenance.py",
                ROOT / "Cargo.toml",
                ROOT / "rust-toolchain.toml",
                ROOT / "uv.lock",
                ROOT / "pyproject.toml",
            ]
        )
    )
    metadata = environment() | {
        "settings": vars(args) | {"output": str(args.output)},
        "cases": CASES,
        "scope": "Complete validation/preparation/grouping/clipping/sorting/scan/remapping/output/FFI. Eager and lazy_complete include plan construction. lazy_plan does not execute. lazy_collect collects a prebuilt plan. streaming_complete builds and collects using the streaming engine. Returned output destruction, fixture construction, correctness and RSS calls are excluded.",
        "memory": "Separate fresh-process eager call. Before/after process working set and peak working set on Windows (RSS/high-water mark elsewhere), includes inputs, runtime and retained output. Peak increase after fixture construction is not exact allocations and may be zero if an earlier high-water mark dominates. No timed RSS polling.",
        "validation": "Every sampled result checked against actual production outside timing. Independent small graph and elementary-cell candidate oracles: benchmarks/test_interval_geometry.py. Large cases use cross-implementation equality, not an independent large oracle.",
        "native_candidate": "Native Polars all-row validation, called directly for eager inputs and behind a blocking no-pushdown map_batches barrier for lazy inputs. Geometry also uses native lazy expressions. Cum-max includes ALL preceding ends. Empty singletons and first-occurrence remapping included.",
        "ordering": "Candidate order rotates per sample. Both methods warmed before samples. Plan-only output schema checked outside timing.",
        "omissions": "No profile baseline: optional coverage-profile comparison omitted to keep the focused A/B/C experiment. Scan-backed cases have no eager scope. Input-generation integer arithmetic is fixture-only. No out-of-core claim.",
        "native_sha256": sha256(native),
        "release_sha256": sha256(release),
        "release_path": str(release),
        "native_path": str(native),
        "build_environment": build_environment(),
        "source_sha256": {str(path.relative_to(ROOT)): sha256(path) for path in sources},
        "status": "running",
    }
    metadata.update(archive_sources(archive, sources))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    memories = []
    with (
        raw.open("w", newline="", encoding="utf-8") as stream,
        tempfile.TemporaryDirectory(prefix="interval-geometry-") as temp,
    ):
        writer = None
        for case_id in args.cases:
            case = CASES[case_id]
            for n in args.sizes:
                for seed in args.seeds:
                    df, options = fixture(n, case, seed)
                    source = df.lazy()
                    if case[-1] == "parquet":
                        path = Path(temp) / "input.parquet"
                        df.write_parquet(path, row_group_size=max(1, n // 7))
                        source = pl.scan_parquet(path)
                    for operation in args.operations:
                        expected = solve(df, operation, "production", options)
                        stats = statistics(df, options, operation, expected)
                        plans = {
                            name: solve(source, operation, name, options)
                            for name in ("production", "native")
                        }
                        for plan in plans.values():
                            assert_frame_equal(plan.collect(), expected)
                        for scope in args.scopes:
                            if scope == "eager" and case[-1] == "parquet":
                                continue

                            def call(
                                method,
                                scope=scope,
                                df=df,
                                operation=operation,
                                options=options,
                                source=source,
                                plans=plans,
                            ):
                                if scope == "eager":
                                    return solve(df, operation, method, options)
                                if scope == "lazy_collect":
                                    return plans[method].collect()
                                plan = solve(source, operation, method, options)
                                if scope == "lazy_plan":
                                    return plan
                                return plan.collect(
                                    engine="streaming" if scope == "streaming_complete" else "auto"
                                )

                            for method in plans:
                                for _ in range(args.warmups):
                                    call(method)
                            for sample in range(args.samples):
                                names = list(plans)
                                names = names[sample % 2 :] + names[: sample % 2]
                                for method in names:
                                    begin = perf_counter_ns()
                                    result = call(method)
                                    elapsed = perf_counter_ns() - begin
                                    if scope == "lazy_plan":
                                        assert result.collect_schema() == expected.schema
                                    else:
                                        assert_frame_equal(result, expected)
                                    del result
                                    row = {
                                        "case": case_id,
                                        **dict(zip(DIMENSIONS, case)),
                                        "seed": seed,
                                        "n": n,
                                        "operation": operation,
                                        "scope": scope,
                                        "method": method,
                                        "sample": sample,
                                        "ns": elapsed,
                                        **stats,
                                    }
                                    if writer is None:
                                        writer = csv.DictWriter(stream, fieldnames=list(row))
                                        writer.writeheader()
                                    writer.writerow(row)
                        if args.memory and case[-1] == "memory":
                            for method in plans:
                                text = subprocess.check_output(
                                    [
                                        sys.executable,
                                        "-m",
                                        "benchmarks.interval_geometry",
                                        "--memory-worker",
                                        str(case_id),
                                        str(n),
                                        str(seed),
                                        operation,
                                        method,
                                    ],
                                    cwd=ROOT,
                                    text=True,
                                )
                                memories.append(json.loads(text))
                    stream.flush()
            print(f"case {case_id}: {case} complete", flush=True)
    if args.memory:
        memory.write_text(json.dumps(memories, indent=2) + "\n", encoding="utf-8")
    metadata["status"] = "complete"
    metadata["raw_sha256"] = sha256(raw)
    metadata.update(source_changes(metadata["source_sha256"]))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
