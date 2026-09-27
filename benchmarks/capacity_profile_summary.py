"""Summarize raw profile measurements using medians, preserving workload keys."""

import sys

import polars as pl


def main():
    paths = sys.argv[1:] or ["benchmarks/results/capacity-profile-core.csv"]
    query = (
        pl.scan_csv(paths)
        .group_by("family", "pattern", "order", "n", "m", "k", "algorithm")
        .agg(
            (pl.col("ns").median() / 1_000_000).alias("median_ms"),
            pl.col("ns").min().alias("min_ns"),
            pl.col("ns").max().alias("max_ns"),
            pl.col("augmentations").first(),
            pl.col("vertices").first(),
            pl.col("edges").first(),
            pl.col("components").first(),
            pl.col("peak_allocated_bytes").max(),
        )
        .sort("n", "family", "pattern", "m", "k", "algorithm")
    )
    sys.stdout.write(query.collect().write_csv())


if __name__ == "__main__":
    main()
