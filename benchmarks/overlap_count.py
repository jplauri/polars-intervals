"""Standalone benchmark; see benchmarks/README.md for methodology and usage."""

import argparse
import json
import os
import platform
import random
import subprocess
from datetime import UTC, datetime
from importlib.metadata import version
from pathlib import Path
from statistics import median
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars.testing import assert_frame_equal


def plugin_query(frame: pl.DataFrame) -> pl.LazyFrame:
    return frame.lazy().select(pi.overlap_count("start", "end").alias("count"))


def join_query(frame: pl.DataFrame) -> pl.LazyFrame:
    rows = frame.lazy().with_row_index("row")
    nonempty = rows.filter(pl.col("start") < pl.col("end"))
    counts = (
        nonempty.join_where(
            nonempty.select("start", "end"),
            pl.col("start") < pl.col("end_right"),
            pl.col("start_right") < pl.col("end"),
        )
        .group_by("row")
        # Every non-empty row matches itself exactly once, even with duplicates.
        .agg((pl.len().cast(pl.UInt64) - 1).alias("count"))
    )
    return (
        rows.select("row")
        .join(counts, on="row", how="left", maintain_order="left")
        .select(pl.col("count").fill_null(0))
    )


def make_frame(scenario: str, size: int, seed: int) -> pl.DataFrame:
    rng = random.Random(seed)
    starts = (
        [4 * i for i in range(size)]
        if scenario == "sparse"
        else [rng.randrange(1000) for _ in range(size)]
    )
    max_length = 8 if scenario == "sparse" else 1000
    intervals = [(start, start + rng.randint(0, max_length)) for start in starts]
    rng.shuffle(intervals)
    return pl.DataFrame(intervals, schema={"start": pl.Int64, "end": pl.Int64}, orient="row")


def check_semantics() -> None:
    """Check the benchmark baseline against fixed expectations before timing."""
    for starts, ends, expected in [
        ([], [], []),
        ([2], [5], [0]),
        ([1, 1], [1, 1], [0, 0]),
        ([3, 1, 2, 1, 5], [5, 3, 2, 3, 6], [0, 1, 0, 1, 0]),
        ([5, 0, 2, 6], [7, 10, 3, 9], [2, 3, 1, 2]),
    ]:
        frame = pl.DataFrame(
            {"start": starts, "end": ends},
            schema={"start": pl.Int64, "end": pl.Int64},
        )
        expected_frame = pl.DataFrame({"count": expected}, schema={"count": pl.UInt64})
        for query in (plugin_query(frame), join_query(frame)):
            assert_frame_equal(query.collect(engine="in-memory"), expected_frame)


def measure(frame: pl.DataFrame, warmups: int, repeats: int) -> dict:
    queries = {"plugin": plugin_query(frame), "inequality_join": join_query(frame)}
    expected = queries["plugin"].collect(engine="in-memory")
    assert_frame_equal(queries["inequality_join"].collect(engine="in-memory"), expected)
    samples = {name: [] for name in queries}
    names = list(queries)
    for iteration in range(warmups + repeats):
        # Alternate order to avoid always timing one method first.
        for name in names[:: 1 if iteration % 2 == 0 else -1]:
            start = perf_counter_ns()
            result = queries[name].collect(engine="in-memory")
            elapsed = (perf_counter_ns() - start) / 1_000_000
            assert_frame_equal(result, expected)  # Outside the timed region.
            del result
            if iteration >= warmups:
                samples[name].append(elapsed)

    directed_overlaps = int(expected["count"].sum())
    nonempty = frame.lazy().select((pl.col("start") < pl.col("end")).sum()).collect().item()
    return {
        "overlapping_pairs": directed_overlaps // 2,
        "overlap_density": (
            directed_overlaps / (frame.height * (frame.height - 1)) if frame.height > 1 else 0.0
        ),
        "join_match_rows_including_self": directed_overlaps + nonempty,
        "empty_intervals": frame.height - nonempty,
        "timings": {
            name: {
                "median_ms": median(values),
                "min_ms": min(values),
                "max_ms": max(values),
                "samples_ms": values,
            }
            for name, values in samples.items()
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sizes", type=int, nargs="+", default=[100, 1000, 3000])
    parser.add_argument(
        "--scenarios",
        nargs="+",
        choices=["sparse", "dense"],
        default=["sparse", "dense"],
    )
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--output", type=Path, default=Path("target/benchmarks/overlap-count.json"))
    args = parser.parse_args()
    if min(args.sizes) < 0 or args.warmups < 0 or args.repeats < 1:
        parser.error("sizes and warmups must be non-negative; repeats must be positive")

    repo = Path(__file__).resolve().parents[1]
    report = {
        "environment": {
            "timestamp_utc": datetime.now(UTC).isoformat(),
            "git_commit": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=repo, text=True
            ).strip(),
            "git_dirty": bool(
                subprocess.check_output(
                    ["git", "status", "--porcelain"], cwd=repo, text=True
                ).strip()
            ),
            "python": platform.python_version(),
            "polars": pl.__version__,
            "polars_intervals": version("polars-intervals"),
            "platform": platform.platform(),
            "processor": platform.processor(),
            "logical_cpus": os.cpu_count(),
            "polars_threads": pl.thread_pool_size(),
            "engine": "in-memory",
        },
        "configuration": {key: value for key, value in vars(args).items() if key != "output"},
        "results": [],
    }
    check_semantics()
    print("scenario  rows     overlap pairs   plugin median ms   join median ms", flush=True)
    for scenario in args.scenarios:
        for size in args.sizes:
            result = measure(make_frame(scenario, size, args.seed), args.warmups, args.repeats)
            result.update(scenario=scenario, rows=size)
            report["results"].append(result)
            timings = result["timings"]
            print(
                f"{scenario:8} {size:7} {result['overlapping_pairs']:15} "
                f"{timings['plugin']['median_ms']:18.3f} "
                f"{timings['inequality_join']['median_ms']:16.3f}",
                flush=True,
            )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Raw timings and environment: {args.output}")


if __name__ == "__main__":
    main()
