"""Compare complete public set operations with the native-Polars event plan.

Run with ``uv run --no-sync python -m benchmarks.set_geometry --output ...``
after rebuilding the release plugin. Inputs and correctness are outside timing.
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
from benchmarks.set_geometry_native import native_set_geometry

# Geometry, independent orders, dtype, groups, matching, chunks, payload, source.
DIMENSIONS = [
    "family",
    "left_order",
    "right_order",
    "dtype",
    "groups",
    "matching",
    "chunks",
    "payload",
    "source",
]
CASES = [
    ("availability", "start", "start", "i64", 0, "all", 1, 0, "memory"),
    ("availability", "shuffled", "reverse", "i64", 0, "all", 1, 0, "memory"),
    ("dense", "shuffled", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("nested", "start", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("duplicates", "shuffled", "reverse", "date", 0, "all", 1, 0, "memory"),
    ("touching", "reverse", "start", "i64", 0, "all", 1, 0, "memory"),
    ("disjoint", "shuffled", "shuffled", "u64", 0, "all", 1, 0, "memory"),
    ("left_heavy", "partial", "reverse", "i64", 0, "all", 7, 0, "memory"),
    ("right_heavy", "start", "shuffled", "i64", 0, "all", 7, 0, "memory"),
    ("empty_right", "shuffled", "start", "i64", 0, "all", 1, 0, "memory"),
    ("availability", "shuffled", "reverse", "datetime_ns", 1000, "all", 7, 0, "memory"),
    ("availability", "partial", "shuffled", "i64", 32, "sparse", 7, 8, "memory"),
    ("genomic", "shuffled", "shuffled", "i64", 8, "all", 7, 0, "parquet"),
    ("skew", "shuffled", "reverse", "i64", 32, "all", 7, 0, "memory"),
    ("sparse", "partial", "reverse", "u64", 0, "all", 1, 0, "memory"),
    ("empty_left", "start", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("empty_rows", "start", "shuffled", "i64", 32, "all", 7, 0, "memory"),
]
SCOPES = (
    "eager",
    "lazy_complete",
    "lazy_plan",
    "lazy_collect",
    "mixed_complete",
    "streaming_complete",
)


def fixture(total, case, seed):
    family, left_order, right_order, dtype, groups, matching, chunks, payload, _ = case
    left_n = total // 2
    if family == "left_heavy":
        left_n = max(0, total - min(8, total))
    elif family == "right_heavy":
        left_n = min(1, total)
    elif family == "empty_right":
        left_n = total
    elif family == "empty_left":
        left_n = 0
    endpoint_dtype = {
        "i64": pl.Int64,
        "u64": pl.UInt64,
        "date": pl.Date,
        "datetime_ns": pl.Datetime("ns", "UTC"),
    }[dtype]

    def side(n, right, order):
        df = pl.DataFrame({"i": pl.int_range(n, eager=True, dtype=pl.Int64)})
        i = pl.col("i")
        start = 10 * i + (2 if right else 0)
        end = start + (4 if right else 8)
        if family in ("dense", "genomic"):
            start = i * (2 if family == "dense" else 7) + right
            end = start + ((i * 37 + seed % 101) % 101) + 1
        elif family == "nested":
            start, end = i, 2 * total - i
        elif family == "duplicates":
            start, end = i % 8, i % 8 + 12
        elif family == "touching":
            start, end = i * 4 + right, i * 4 + right + 4
        elif family == "disjoint":
            start, end = i * 10 + (5 if right else 0), i * 10 + (8 if right else 3)
        elif family == "right_heavy" and not right:
            start, end = pl.lit(0), pl.lit(total * 10)
        elif family == "left_heavy" and right:
            start, end = i * (total + 1), i * (total + 1) + total // 2
        elif family == "sparse":
            start = i * 10_000_000_000 + right * 2
            end = start + (4 if right else 8)
        elif family == "empty_rows":
            end = start
        if dtype == "u64":
            start, end = start.cast(pl.UInt64) + 2**63, end.cast(pl.UInt64) + 2**63
        df = df.with_columns(
            start.cast(endpoint_dtype).alias("start"), end.cast(endpoint_dtype).alias("end")
        )
        if groups:
            group = i % groups
            if matching == "sparse" and right:
                group = group + groups - 1
            group = pl.when(i % 19 == 0).then(None).otherwise(group)
            if family == "skew":
                group = pl.when(i < n * 3 // 4).then(None).otherwise(group)
            df = df.with_columns(group.cast(pl.Int64).alias("group"))
        if order == "reverse":
            df = df.reverse()
        elif order == "shuffled":
            df = df.sample(fraction=1, shuffle=True, seed=(seed + right) % 2**64)
        elif order == "partial":
            df = df.sort(pl.col("i") // 64, -(pl.col("i") % 64))
        if payload:
            df = df.with_columns(
                *[pl.lit("irrelevant payload " * 20).alias(f"payload{i}") for i in range(payload)]
            )
        df = df.drop("i")
        if chunks > 1 and n:
            df = pl.concat(
                [df.slice(i, max(1, n // chunks)) for i in range(0, n, max(1, n // chunks))],
                rechunk=False,
            )
        return df

    return (
        side(left_n, False, left_order),
        side(total - left_n, True, right_order),
        {"by": "group" if groups else None},
    )


def solve(left, right, operation, method, options):
    if method == "native":
        return native_set_geometry(left, right, intersection=operation == "intersect", **options)
    return (pi.intersect_intervals if operation == "intersect" else pi.subtract_intervals)(
        left, right, **options
    )


def statistics(left, right, options, output):
    by = options["by"]
    sizes = left.group_by(by).len()["len"] if by else pl.Series([left.height])
    keys_left = left.select(by).unique() if by else None
    matched = (
        keys_left.join(right.select(by).unique(), on=by, nulls_equal=True).height
        if by
        else int(bool(left.height and right.height))
    )
    return {
        "left_n": left.height,
        "right_n": right.height,
        "left_useful": left.select((pl.col("start") < pl.col("end")).sum()).item(),
        "right_useful": right.select((pl.col("start") < pl.col("end")).sum()).item(),
        "p": pi.merge_intervals(left, **options).height,
        "q": pi.merge_intervals(right, **options).height,
        "z": output.height,
        "distinct_endpoints": pl.concat(
            [frame[name] for frame in (left, right) for name in ("start", "end")]
        ).n_unique(),
        "left_groups": len(sizes) if by else int(left.height > 0),
        "matched_groups": matched,
        "key_match_fraction": matched / len(sizes) if by and len(sizes) else int(bool(matched)),
        "largest_left_group": sizes.max() or 0,
        "largest_left_group_fraction": (sizes.max() or 0) / left.height if left.height else 0,
        "left_chunks": left["start"].n_chunks(),
        "right_chunks": right["start"].n_chunks(),
    }


def memory_worker(values):
    case, n, seed, operation, method = values
    left, right, options = fixture(int(n), CASES[int(case)], int(seed))
    before = resident_memory()
    result = solve(left, right, operation, method, options)
    after = resident_memory()
    print(
        json.dumps(
            {
                "case": int(case),
                "n": int(n),
                "seed": int(seed),
                "operation": operation,
                "method": method,
                "before": before,
                "after": after,
                "peak_increase_bytes": max(0, after["peak_rss_bytes"] - before["peak_rss_bytes"]),
                "z": result.height,
            }
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--sizes", type=int, nargs="+", default=[8, 1000, 10000, 100000])
    parser.add_argument("--seeds", type=int, nargs="+", default=[7, 41])
    parser.add_argument(
        "--cases", type=int, nargs="+", choices=range(len(CASES)), default=list(range(len(CASES)))
    )
    parser.add_argument("--scopes", nargs="+", choices=SCOPES, default=["eager", "lazy_complete"])
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--memory", action="store_true")
    parser.add_argument("--memory-worker", nargs=5, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.memory_worker:
        memory_worker(args.memory_worker)
        return
    if args.output is None or min(args.sizes) < 0 or min(args.samples, args.warmups) < 1:
        parser.error("Need output, nonnegative sizes and positive samples/warmups")
    if any(seed < 0 or seed > 2**64 - 1 for seed in args.seeds):
        parser.error("Seeds must fit UInt64")
    native = Path(_internal.__file__)
    try:
        release, inputs = verify_release(native)
    except ValueError as error:
        parser.error(str(error))
    raw, meta, archive, memory = [
        args.output.with_suffix(ext)
        for ext in (".csv", ".metadata.json", ".sources.zip", ".memory.json")
    ]
    if any(path.exists() for path in (raw, meta, archive, memory)):
        parser.error("Use a new output prefix")
    raw.parent.mkdir(parents=True, exist_ok=True)
    sources = [
        *inputs,
        *ROOT.glob("python/polars_intervals/*.py"),
        *ROOT.glob("benchmarks/set_geometry*.py"),
        ROOT / "benchmarks/test_set_geometry.py",
        ROOT / "benchmarks/provenance.py",
        ROOT / "uv.lock",
        ROOT / "pyproject.toml",
    ]
    metadata = environment() | {
        "settings": vars(args) | {"output": str(args.output)},
        "cases": CASES,
        "scope": "Both-side validation, preparation, grouping, sorting, set scan/events, output construction and native crossing. Eager/lazy_complete/mixed_complete/streaming_complete include planning. lazy_plan excludes execution; lazy_collect uses a prebuilt plan. Input construction, correctness, returned-output destruction and memory observations excluded.",
        "memory": "Separate cold process eager call: before/after RSS and process peak RSS, including runtime, inputs and retained output. Peak increase after fixture creation is not allocation count or algorithm heap usage.",
        "verification": "Small original-row Boolean cell oracle in benchmarks/test_set_geometry.py. Every measured complete output checked outside timing against production. Large instances use cross-implementation agreement, not an independent large oracle.",
        "native_candidate": "D: all-row Polars validation behind a whole-combined-input blocking barrier, grouped signed endpoint events, per-source cumulative counts, Boolean membership and gap-aware coalescing. No Python row/group loops or nested collect.",
        "ordering": "Two warmups by default; candidate order rotates per sample; both methods fully materialize outputs.",
        "limitations": "Synthetic single-machine in-memory and separately labeled warm Parquet workloads. No out-of-core claim. No all-pairs join comparator or CPU hardware counters.",
        "native_path": str(native),
        "native_sha256": sha256(native),
        "release_path": str(release),
        "release_sha256": sha256(release),
        "build_environment": build_environment(),
        "source_sha256": {str(path.relative_to(ROOT)): sha256(path) for path in sources},
        "status": "running",
    }
    metadata.update(archive_sources(archive, sources))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    memories = []
    with (
        raw.open("w", newline="", encoding="utf-8") as stream,
        tempfile.TemporaryDirectory(prefix="set-geometry-") as temp,
    ):
        writer = None
        for case_id in args.cases:
            case = CASES[case_id]
            for n in args.sizes:
                for seed in args.seeds:
                    left, right, options = fixture(n, case, seed)
                    lhs, rhs = left.lazy(), right.lazy()
                    if case[-1] == "parquet":
                        paths = [Path(temp) / f"{side}.parquet" for side in ("left", "right")]
                        for frame, path, divisor in zip((left, right), paths, (7, 11)):
                            frame.write_parquet(
                                path, row_group_size=max(1, frame.height // divisor)
                            )
                        lhs, rhs = [pl.scan_parquet(path) for path in paths]
                    for operation in ("subtract", "intersect"):
                        expected = solve(left, right, operation, "production", options)
                        stats = statistics(left, right, options, expected)
                        plans = {
                            method: solve(lhs, rhs, operation, method, options)
                            for method in ("production", "native")
                        }
                        for plan in plans.values():
                            assert_frame_equal(plan.collect(), expected)
                        for scope in args.scopes:
                            if case[-1] == "parquet" and scope in ("eager", "mixed_complete"):
                                continue

                            def call(
                                method,
                                scope=scope,
                                left=left,
                                right=right,
                                operation=operation,
                                options=options,
                                lhs=lhs,
                                rhs=rhs,
                                plans=plans,
                            ):
                                if scope == "eager":
                                    return solve(left, right, operation, method, options)
                                if scope == "lazy_collect":
                                    return plans[method].collect()
                                plan = solve(
                                    left if scope == "mixed_complete" else lhs,
                                    rhs,
                                    operation,
                                    method,
                                    options,
                                )
                                if scope == "lazy_plan":
                                    return plan
                                return plan.collect(
                                    engine="streaming" if scope == "streaming_complete" else "auto"
                                )

                            for method in plans:
                                for _ in range(args.warmups):
                                    call(method)
                            for sample in range(args.samples):
                                methods = list(plans) if sample % 2 == 0 else list(reversed(plans))
                                for method in methods:
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
                                        "benchmarks.set_geometry",
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
        metadata["memory_sha256"] = sha256(memory)
    metadata.update(status="complete", raw_sha256=sha256(raw))
    metadata.update(source_changes(metadata["source_sha256"]))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
