"""Release-plugin nesting depth timings with closed-form and quadratic checks.

Run sequentially after building/installing a release wheel. Fixture construction,
casts, expression construction and correctness checks are outside the timer.
Lazy optimization/planning, collection and result Series retrieval are timed.
"""

import argparse
import csv
import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from provenance import environment, sha256


def fixture(n, groups, family, order, dtype):
    """Analytic depth travels with the row through each ordering transformation."""
    local = pl.col("i") // groups
    if family == "chain":
        start, end, depth = local, 2 * ((n + groups - 1) // groups) - local, local
    elif family == "crossing":
        start, end, depth = local, local + n + 1, pl.lit(0)
    else:
        levels = 8 if family == "low8" else 64
        copies = 2 if family == "duplicate64" else 1
        level = local % (levels * copies) // copies
        base = local // (levels * copies) * (4 * levels)
        start, end, depth = base + level, base + 2 * levels - level, level
    frame = (
        pl.LazyFrame({"i": range(n)})
        .select(
            start=start,
            end=end,
            group=pl.col("i") % groups,
            expected=depth.cast(pl.UInt64),
        )
        .collect()
    )
    if order == "shuffled":
        frame = frame.sample(fraction=1, shuffle=True, seed=731)
    elif order == "reverse":
        frame = frame.reverse()
    elif order == "nearly":
        # Swap distinct geometries, even when the final rows are duplicates.
        swap_at = n - 2
        while swap_at >= 0 and frame.row(swap_at)[:2] == frame.row(swap_at + 1)[:2]:
            swap_at -= 1
        if swap_at >= 0:
            frame = pl.concat(
                [frame.head(swap_at), frame.slice(swap_at, 2).reverse(), frame.slice(swap_at + 2)]
            )
    if dtype == "Date":
        cast = (pl.col("start", "end") + 20_000).cast(pl.Date)
    elif dtype == "Datetime(us)":
        cast = (pl.col("start", "end") * 1_000_000 + 1_735_689_600_000_000).cast(pl.Datetime("us"))
    else:
        cast = pl.col("start", "end").cast(pl.Int64)
    return frame.lazy().with_columns(cast).collect()


def quadratic(frame):
    rows = frame.lazy().select(pl.col("start", "end").to_physical(), "group").collect()
    starts, ends, groups = (rows[name].to_list() for name in ("start", "end", "group"))
    depths = [0] * len(starts)
    for j in sorted(range(len(starts)), key=lambda i: (starts[i], -ends[i], i)):
        depths[j] = max(
            (
                depths[i] + 1
                for i in range(len(starts))
                if groups[i] == groups[j]
                and starts[i] <= starts[j]
                and ends[j] <= ends[i]
                and (starts[i] < starts[j] or ends[j] < ends[i])
            ),
            default=0,
        )
    return depths


def query(frame, groups):
    expr = pi.nesting_depth("start", "end")
    if groups > 1:
        expr = expr.over("group")
    return frame.lazy().select(expr.alias("depth"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("target/nesting-polars.csv"))
    parser.add_argument(
        "--sizes", nargs="+", type=int, default=[1000, 10000, 100000, 1000000, 3000000]
    )
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--group-size", type=int, default=100000)
    args = parser.parse_args()
    if args.repeats < 1 or args.group_size < 1 or min(args.sizes) < 1:
        parser.error("sizes and repeats must be positive")
    families = ["crossing", "low8", "moderate64", "chain", "duplicate64"]
    for family in families:
        for groups in (1, 10):
            for dtype in ("Int64", "Date", "Datetime(us)"):
                frame = fixture(193, groups, family, "shuffled", dtype)
                expected = quadratic(frame)
                assert expected == frame["expected"].to_list()
                assert query(frame, groups).collect()["depth"].to_list() == expected
    cases = [
        (n, 1, family, order, "Int64")
        for n in args.sizes
        for family in families
        for order in ("sorted", "reverse", "nearly", "shuffled")
    ]
    # Temporal physical kernels are i32/i64, so avoid a redundant full matrix.
    cases += [
        (n, 1, family, "shuffled", dtype)
        for n in args.sizes
        for family in ("low8", "chain")
        for dtype in ("Date", "Datetime(us)")
    ]
    cases += [
        (args.group_size, groups, family, "shuffled", dtype)
        for groups in (1, 10, 100, 1000)
        for family in ("crossing", "low8", "chain", "duplicate64")
        for dtype in ("Int64", "Date", "Datetime(us)")
    ]
    cases = list(dict.fromkeys(cases))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    native_files = list(Path(pi.__file__).parent.glob("*.pyd")) + list(
        Path(pi.__file__).parent.glob("*.so")
    )
    metadata = {
        **environment(),
        "working_directory": str(Path.cwd()),
        "plugin": pi.__file__,
        "native_sha256": {p.name: sha256(p) for p in native_files},
        "runner_sha256": sha256(Path(__file__)),
        "cases": len(cases),
        "warmups": 1,
        "samples": args.repeats,
        "seed": 731,
        "timed_scope": (
            "lazy optimization/planning, collect and result Series retrieval; "
            "no fixture construction, casting, expression construction or validation"
        ),
        "baseline": "No equivalent small composition of native expressions identified; no speedup claim.",
        "memory": "Not measured here; core allocation measurements are separate.",
    }
    args.output.with_suffix(".json").write_text(json.dumps(metadata, indent=2) + "\n")
    with args.output.open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(
            [
                "n",
                "groups",
                "family",
                "order",
                "dtype",
                "candidate",
                "sample",
                "total_ns",
                "max_depth",
            ]
        )
        for n, groups, family, order, dtype in cases:
            frame = fixture(n, groups, family, order, dtype)
            expected = frame["expected"].rename("depth")
            plan = query(frame, groups)
            for sample in range(-1, args.repeats):
                tick = perf_counter_ns()
                result = plan.collect()["depth"]
                elapsed = perf_counter_ns() - tick
                assert result.dtype == pl.UInt64 and result.null_count() == 0
                assert result.equals(expected)
                if sample >= 0:
                    writer.writerow(
                        [n, groups, family, order, dtype, "plugin", sample, elapsed, result.max()]
                    )
            file.flush()
            print(
                f"{n} rows, {groups} groups, {family}, {order}, {dtype}: verified", file=sys.stderr
            )
    metadata["completed_utc"] = datetime.now(UTC).isoformat()
    metadata["validated_samples"] = len(cases) * args.repeats
    metadata["exit_code"] = 0
    args.output.with_suffix(".json").write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    main()
