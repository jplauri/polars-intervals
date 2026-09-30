"""Complete release-plugin scheduling queries with independent untimed checks.

Rebuild the release plugin first, then run with ``uv run --no-sync python``.
The default Polars thread setting is retained and recorded in the run metadata.
"""

import argparse
import csv
import json
import random
from bisect import bisect_left
from importlib.metadata import version
from itertools import pairwise
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from balance_lanes import validate_coloring
from polars_intervals import _internal
from provenance import (
    ROOT,
    archive_sources,
    build_environment,
    environment,
    sha256,
    source_changes,
    verify_release,
)

FAMILIES = ("disjoint", "moderate128", "nested", "random_lengths")
OPERATIONS = ("assign_lanes", "max_weight_non_overlapping")


def fixture(n, family, order, seed):
    rng = random.Random(seed)
    rows = []
    for i in range(n):
        start, end = {
            "disjoint": (3 * i, 3 * i + 2),
            "moderate128": (i, i + 128),
            "nested": (i, 2 * n - i),
            "random_lengths": (i, i + 1 + rng.randrange(max(1, n // 8))),
        }[family]
        rows.append((start, end, rng.randrange(1, 101)))
    if order == "shuffled":
        rng.shuffle(rows)
    else:
        rows.sort()
    frame = pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "weight": pl.Int64}, orient="row"
    )
    return frame, rows


def weighted_optimum(rows):
    """Independent start-ordered suffix DP, separate from production finish ordering."""
    nonempty = sorted(row for row in rows if row[0] < row[1])
    starts = [start for start, _, _ in nonempty]
    suffix = [0] * (len(nonempty) + 1)
    for i in range(len(nonempty) - 1, -1, -1):
        _, end, weight = nonempty[i]
        suffix[i] = max(suffix[i + 1], weight + suffix[bisect_left(starts, end)])
    return suffix[0] + sum(max(0, weight) for start, end, weight in rows if start == end)


def check(rows, operation, result):
    output = result.to_series()
    assert output.dtype == (pl.UInt32 if operation == "assign_lanes" else pl.Boolean)
    assert len(output) == len(rows) and output.null_count() == 0
    values = output.to_list()
    if operation == "assign_lanes":
        objective = validate_coloring([row[:2] for row in rows], values)["k"]
        assert all(lane == 0 for (start, end, _), lane in zip(rows, values) if start == end)
        return objective
    chosen = [row for row, selected in zip(rows, values, strict=True) if selected]
    assert all(weight > 0 for _, _, weight in chosen)
    assert all(
        selected
        for (start, end, weight), selected in zip(rows, values, strict=True)
        if start == end and weight > 0
    )
    intervals = sorted((start, end) for start, end, _ in chosen if start < end)
    assert all(left[1] <= right[0] for left, right in pairwise(intervals))
    objective = sum(weight for _, _, weight in chosen)
    assert objective == weighted_optimum(rows)
    return objective


def solve(frame, operation):
    expression = (
        pi.assign_lanes("start", "end")
        if operation == "assign_lanes"
        else pi.max_weight_non_overlapping("start", "end", weight="weight")
    )
    return frame.lazy().select(expression.alias("result")).collect()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New output prefix")
    parser.add_argument("--sizes", type=int, nargs="+", default=[1000, 100000, 1000000])
    parser.add_argument("--seeds", type=int, nargs="+", default=[7, 41])
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    args = parser.parse_args()
    if min(args.sizes) < 0 or args.samples < 1 or args.warmups < 1:
        parser.error("Need nonnegative sizes and positive sample/warmup counts")
    native = Path(_internal.__file__)
    try:
        release, inputs = verify_release(native)
    except ValueError as error:
        parser.error(str(error))
    raw = args.output.with_suffix(".csv")
    meta = args.output.with_suffix(".metadata.json")
    archive = args.output.with_suffix(".sources.zip")
    if any(path.exists() for path in (raw, meta, archive)):
        parser.error("Use a new output prefix to preserve existing measurements")
    raw.parent.mkdir(parents=True, exist_ok=True)
    sources = [
        *inputs,
        *ROOT.glob("python/polars_intervals/*.py"),
        *(
            ROOT / "benchmarks" / name
            for name in (
                "scheduling_polars.py",
                "test_scheduling_polars.py",
                "balance_lanes.py",
                "balance_lanes_fixtures.py",
                "generate_interval_graphs.py",
                "provenance.py",
            )
        ),
        ROOT / "pyproject.toml",
        ROOT / "uv.lock",
    ]
    metadata = environment() | {
        "settings": vars(args) | {"output": str(args.output)},
        "package_version": version("polars-intervals"),
        "package_path": pi.__file__,
        "families": FAMILIES,
        "operations": OPERATIONS,
        "scope": "Complete lazy query: public expression/query construction, optimization, plugin validation/extraction, scheduling and row-aligned output. Fixture/input construction, oracle/checks and output destruction excluded.",
        "verification": "Initial output independently checked for lane feasibility/minimum palette or weighted feasibility/optimal objective with a start-ordered suffix DP. Every warmup and timed output compared against that validated deterministic result outside timing.",
        "ordering": "Sorted means by start, then end and weight. Shuffled uses a seeded permutation of the same rows. Operation order rotates between samples.",
        "threads": "No thread environment is changed by the runner. Default runs leave POLARS_MAX_THREADS unset; actual settings and pool size are recorded above.",
        "coverage": "Synthetic ungrouped Int64 endpoints, positive Int64 weights, one chunk per column, auto collection engine. No native Polars baseline or memory measurement.",
        "objective": "Minimum lane count for assign_lanes; maximum selected total weight for max_weight_non_overlapping.",
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
    count = 0
    with raw.open("x", newline="", encoding="utf-8") as stream:
        writer = csv.writer(stream)
        writer.writerow(
            [
                "family",
                "order",
                "seed",
                "scope",
                "dtype",
                "weights",
                "operation",
                "method",
                "n",
                "sample",
                "ns",
                "objective",
            ]
        )
        for n in args.sizes:
            for seed in args.seeds:
                for family in FAMILIES:
                    for order in ("sorted", "shuffled"):
                        frame, rows = fixture(n, family, order, seed)
                        expected, objectives = {}, {}
                        for operation in OPERATIONS:
                            expected[operation] = solve(frame, operation)
                            objectives[operation] = check(rows, operation, expected[operation])
                            for _ in range(args.warmups - 1):
                                result = solve(frame, operation)
                                assert result.equals(expected[operation])
                                del result
                        for sample in range(args.samples):
                            for operation in OPERATIONS[sample % 2 :] + OPERATIONS[: sample % 2]:
                                begin = perf_counter_ns()
                                result = solve(frame, operation)
                                elapsed = perf_counter_ns() - begin
                                assert result.equals(expected[operation])
                                del result
                                writer.writerow(
                                    [
                                        family,
                                        order,
                                        seed,
                                        "lazy_complete",
                                        "i64",
                                        "positive",
                                        operation,
                                        "production",
                                        n,
                                        sample,
                                        elapsed,
                                        objectives[operation],
                                    ]
                                )
                                count += 1
                        stream.flush()
                        del frame, rows, expected, objectives
                        print(f"Checked n={n}, seed={seed}, {family}, {order}", flush=True)
    metadata.update(source_changes(metadata["source_sha256"]))
    metadata.update(status="completed", samples=count, samples_sha256=sha256(raw))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"Saved {count} samples to {raw}")


if __name__ == "__main__":
    main()
