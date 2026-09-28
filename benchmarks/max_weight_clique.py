"""Complete release-plugin collections; fixtures and correctness checks are untimed.

Rebuild using docs/benchmarking.md, then run with ``uv run --no-sync python``.
The installed binary must match the local Cargo release library byte-for-byte.
No new benchmark or property-testing dependency is used.
"""

from __future__ import annotations

import argparse
import csv
import json
import random
from collections import defaultdict
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars_intervals import _internal
from provenance import ROOT, environment, sha256

# Focused workload matrix, not a Cartesian product. Group IDs are interleaved.
CASES = [
    ("low8", "sorted", "i64", 1, 1, "auto"),
    ("low8", "shuffled", "i64", 1, 1, "auto"),
    ("disjoint", "shuffled", "u64", 1, 1, "auto"),
    ("nested", "reverse", "i64", 1, 1, "auto"),
    ("clique", "shuffled", "date", 1, 1, "auto"),
    ("repeated", "shuffled", "datetime_us", 1, 1, "auto"),
    ("low8", "shuffled", "datetime_ns_utc", 1, 7, "streaming"),
    ("empty", "sorted", "i64", 1, 1, "auto"),
    ("mixed_empty", "shuffled", "i64", 1, 7, "auto"),
    ("low8", "shuffled", "i64", 32, 7, "auto"),
    ("repeated", "shuffled", "date", 32, 1, "streaming"),
]


def fixture(n, case, seed):
    family, order, dtype, groups, chunks, _ = case
    rows = []
    for i in range(n):
        j = i // groups
        start, end = {
            "low8": (j, j + 8),
            "disjoint": (j * 3, j * 3 + 2),
            "nested": (j, 2 * n - j + 1),
            "clique": (0, n + 1),
            "repeated": (j // 64 * 4, j // 64 * 4 + 3),
            "empty": (j, j),
            "mixed_empty": (j, j if j % 4 else j + 8),
        }[family]
        rows.append((start, end, i % groups, i % 23 + 1, (i % 11) - 7))
    if order == "shuffled":
        random.Random(seed).shuffle(rows)
    elif order == "reverse":
        rows.reverse()
    frame = pl.DataFrame(
        rows,
        schema={
            "start": pl.Int64,
            "end": pl.Int64,
            "group": pl.Int64,
            "positive": pl.Int64,
            "mixed": pl.Int64,
        },
        orient="row",
    ).with_columns(pl.lit(1, dtype=pl.Int64).alias("ones"))
    target, offset = {
        "i64": (pl.Int64, 0),
        "u64": (pl.UInt64, 2**64 - 10_000_000),
        "date": (pl.Date, 20_000),
        "datetime_us": (pl.Datetime("us"), 2**53 + 1),
        "datetime_ns_utc": (pl.Datetime("ns", "UTC"), 2**53 + 1),
    }[dtype]
    frame = (
        frame.lazy()
        .with_columns(
            (pl.col("start", "end").cast(pl.UInt64 if dtype == "u64" else pl.Int64) + offset).cast(
                target
            )
        )
        .collect()
    )
    if chunks > 1 and n:
        # Different boundaries in each column, including across the winning clique.
        columns = []
        for column, series in enumerate(frame):
            step = max(1, n // chunks + column)
            columns.append(
                pl.concat([series[k : k + step] for k in range(0, n, step)], rechunk=False)
            )
        frame = pl.DataFrame(columns)
    return frame, rows


def check(rows, mode, mask):
    """Independent Python integer event scores and linear common-point checks."""
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(rows)
    grouped = defaultdict(list)
    for row, selected in zip(rows, mask.to_list(), strict=True):
        start, end, group, positive, mixed = row
        weight = {"unit": 1, "explicit_unit": 1, "positive": positive, "mixed": mixed}[mode]
        grouped[group].append((start, end, weight, selected))
    total = 0
    for group in grouped.values():
        changes = defaultdict(int)
        best_empty = 0
        chosen = []
        for s, e, w, selected in group:
            if selected:
                assert w > 0
                chosen.append((s, e, w))
            if w > 0:
                if s == e:
                    best_empty = max(best_empty, w)
                else:
                    changes[s] += w
                    changes[e] -= w
        active, optimum = 0, best_empty
        for point in sorted(changes):
            active += changes[point]
            optimum = max(optimum, active)
        assert sum(w for _, _, w in chosen) == optimum
        if len(chosen) > 1:
            assert max(s for s, _, _ in chosen) < min(e for _, e, _ in chosen)
            assert all(s < e for s, e, _ in chosen)
        total += optimum
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New output prefix")
    parser.add_argument(
        "--sizes", type=int, nargs="+", default=[0, 8, 64, 1000, 10000, 100000, 1000000]
    )
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 137])
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--release-library", type=Path)
    args = parser.parse_args()
    native = Path(_internal.__file__)
    libraries = [
        ROOT / "target/release" / name
        for name in ("polars_intervals.dll", "libpolars_intervals.so", "libpolars_intervals.dylib")
    ]
    release = args.release_library or next((p for p in libraries if p.exists()), None)
    if release is None or sha256(native) != sha256(release):
        parser.error("Installed plugin does not match Cargo release library; rebuild release first")
    native_inputs = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    for crate in (ROOT / "crates").iterdir():
        native_inputs.append(crate / "Cargo.toml")
        native_inputs.extend((crate / "src").rglob("*.rs"))
    if any(path.stat().st_mtime_ns > release.stat().st_mtime_ns for path in native_inputs):
        parser.error("Rust inputs are newer than the release library; rebuild release first")
    timing_path = args.output.with_suffix(".timings.csv")
    metadata_path = args.output.with_suffix(".metadata.json")
    if timing_path.exists() or metadata_path.exists():
        parser.error("Output exists; choose a new prefix to preserve raw measurements")
    if args.samples < 1 or args.warmups < 1 or min(args.sizes) < 0:
        parser.error("Need nonnegative sizes and positive sample/warmup counts")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    metadata = {
        **environment(),
        "native_sha256": sha256(native),
        "release_library": str(release),
        "release_sha256": sha256(release),
        "native_input_sha256": {
            str(path.relative_to(ROOT)): sha256(path) for path in sorted(native_inputs)
        },
        "source_sha256": {
            str(p.relative_to(ROOT)): sha256(p)
            for p in [
                ROOT / "benchmarks/max_weight_clique.py",
                ROOT / "crates/intervals-core/src/clique.rs",
                ROOT / "crates/polars-intervals/src/lib.rs",
                ROOT / "python/polars_intervals/__init__.py",
                ROOT / "Cargo.lock",
                ROOT / "uv.lock",
            ]
        },
        "sizes": args.sizes,
        "seeds": args.seeds,
        "samples": args.samples,
        "warmups": args.warmups,
        "cases": CASES,
        "scope": "Prepared lazy query collect: optimization, plugin validation/extraction, algorithm, Boolean output. Fixture and expression construction, output checks and destruction excluded.",
        "verification": "Independent Python exact event objective per group, positive-only mask, length/dtype/null checks and linear common-intersection feasibility; repeated masks checked outside timing.",
        "memory": "Not measured in Python; Rust runner separately measures requested live heap.",
        "chunks": "Requested split target, not actual chunk count. Per-column steps are max(1, n // target + column_index), yielding different boundaries/counts; zero-row inputs remain empty.",
        "status": "running",
    }
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    count = 0
    with timing_path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.writer(output)
        writer.writerow(
            [
                "family",
                "order",
                "dtype",
                "groups",
                "chunks",
                "engine",
                "n",
                "seed",
                "weights",
                "algorithm",
                "sample",
                "ns",
                "objective",
            ]
        )
        for n in args.sizes:
            for seed in args.seeds:
                for case in CASES:
                    frame, rows = fixture(n, case, seed)
                    queries, expected, objectives = {}, {}, {}
                    for mode, weight in [
                        ("unit", None),
                        ("explicit_unit", "ones"),
                        ("positive", "positive"),
                        ("mixed", "mixed"),
                    ]:
                        expr = pi.max_weight_clique("start", "end", weight=weight)
                        if case[3] > 1:
                            expr = expr.over("group")
                        query = frame.lazy().select(expr.alias("selected"))
                        queries[mode] = query
                        expected[mode] = query.collect(engine=case[5])
                        objectives[mode] = check(rows, mode, expected[mode].to_series())
                        for _ in range(args.warmups - 1):
                            assert query.collect(engine=case[5]).equals(expected[mode])
                    assert expected["unit"].equals(expected["explicit_unit"])
                    modes = list(queries)
                    for sample in range(args.samples):
                        for mode in modes[sample % 4 :] + modes[: sample % 4]:
                            begin = perf_counter_ns()
                            result = queries[mode].collect(engine=case[5])
                            elapsed = perf_counter_ns() - begin
                            assert result.equals(expected[mode])
                            del result  # Destruction is deliberately outside the timed region.
                            writer.writerow(
                                [
                                    *case,
                                    n,
                                    seed,
                                    mode,
                                    "production",
                                    sample,
                                    elapsed,
                                    objectives[mode],
                                ]
                            )
                            count += 1
                    output.flush()
    metadata.update(status="completed", timing_samples=count, timing_sha256=sha256(timing_path))
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"Saved {count} samples to {timing_path}")


if __name__ == "__main__":
    main()
