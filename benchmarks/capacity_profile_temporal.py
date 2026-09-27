"""Installed release-native profile timings, with an independent clique oracle.

Run after installing the release wheel:
    python benchmarks/capacity_profile_temporal.py > results.csv

No dataframe expression baseline is claimed for this global optimizer. Input
construction, dtype conversion, and independent verification are untimed.
"""

import csv
import os
import sys
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi


def main():
    output = csv.writer(sys.stdout, lineterminator="\n")
    output.writerow(["n", "m", "pattern", "dtype", "algorithm", "sample", "ns", "objective"])
    max_n = int(os.environ.get("PROFILE_BENCH_MAX_N", "1000000"))
    for n in (1_000, 10_000, 100_000, 1_000_000):
        if n > max_n:
            continue
        count = (n + 31) // 32
        source = (
            pl.LazyFrame({"i": range(n)})
            .select(start=pl.col("i") // 32 * 20, weight=pl.col("i") % 32 + 1)
            .with_columns(end=pl.col("start") + 10)
            .collect()
        )
        for pattern in ("constant", "varying", "high"):
            if pattern == "varying":
                profile = (
                    pl.LazyFrame({"i": range(count)})
                    .select(start=pl.col("i") * 20, capacity=(pl.col("i") % 4 + 1) * 2)
                    .with_columns(end=pl.col("start") + 10)
                    .collect()
                )
                capacities = [(j % 4 + 1) * 2 for j in range(count)]
            else:
                capacity = 4 if pattern == "constant" else n
                capacities = [capacity] * count
                profile = pl.DataFrame({"start": [0], "end": [count * 20], "capacity": [capacity]})
            expected = sum(
                sum(range(size + 1 - min(size, capacities[j]), size + 1))
                for j in range(count)
                for size in [min(32, n - j * 32)]
            )
            for dtype, scale, offset in (
                (pl.Int64, 1, 0),
                (pl.Date, 1, 20_000),
                (pl.Datetime("us"), 1_000_000, 1_735_689_600_000_000),
                (pl.Datetime("ns", "UTC"), 1_000_000_000, 1_735_689_600_000_000_000),
            ):
                jobs = (
                    source.lazy()
                    .with_columns(((pl.col("start", "end") * scale) + offset).cast(dtype))
                    .collect()
                )
                capacity_profile = (
                    profile.lazy()
                    .with_columns(((pl.col("start", "end") * scale) + offset).cast(dtype))
                    .collect()
                )
                methods = ["profile", "scalar"] if pattern != "varying" else ["profile"]
                for method in methods:
                    for sample in range(-1, 3):
                        begin = perf_counter_ns()
                        if method == "profile":
                            mask = pi.max_weight_with_capacity_profile(
                                jobs, capacity_profile, weight="weight"
                            )
                        else:
                            mask = jobs.select(
                                pi.max_weight_with_capacity(
                                    "start", "end", weight="weight", capacity=capacities[0]
                                )
                            ).to_series()
                        elapsed = perf_counter_ns() - begin
                        assert mask.dtype == pl.Boolean
                        assert len(mask) == n
                        assert mask.null_count() == 0
                        objective = (
                            source.lazy()
                            .filter(mask)
                            .select(pl.col("weight").sum())
                            .collect()
                            .item()
                        )
                        assert objective == expected
                        counts = (
                            source.lazy()
                            .filter(mask)
                            .group_by("start")
                            .len()
                            .sort("start")
                            .collect()
                        )
                        assert all(
                            active <= capacities[start // 20]
                            for start, active in counts.iter_rows()
                        )
                        if sample >= 0:
                            output.writerow(
                                [
                                    n,
                                    profile.height,
                                    pattern,
                                    str(dtype),
                                    method,
                                    sample,
                                    elapsed,
                                    objective,
                                ]
                            )


if __name__ == "__main__":
    main()
