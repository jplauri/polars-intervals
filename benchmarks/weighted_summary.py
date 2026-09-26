"""Summarize weighted scheduling samples: python benchmarks/weighted_summary.py CSV."""

import argparse

import polars as pl


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv")
    args = parser.parse_args()
    keys = ["family", "order", "weights", "n"]
    samples = pl.scan_csv(args.csv, schema_overrides={"objective": pl.String})
    medians = samples.group_by(*keys, "algorithm").agg(
        pl.col("ns", "preprocessing_ns", "optimization_ns", "reconstruction_ns").median(),
        pl.col("peak_buffer_bytes", "buffer_allocations").first(),
    )
    a = medians.filter(pl.col("algorithm") == "A_binary").select(*keys, pl.col("ns").alias("a_ns"))
    compared = medians.join(a, on=keys).with_columns(
        (pl.col("ns") / pl.col("a_ns")).alias("relative_to_a")
    )
    print("Geometric mean runtime relative to A (smaller is faster):")
    print(
        compared.group_by("n", "order", "algorithm")
        .agg(
            pl.col("relative_to_a").log().mean().exp().alias("ratio"),
        )
        .sort("n", "order", "algorithm")
        .collect()
    )
    print("Overall geometric mean relative to A:")
    print(
        compared.group_by("algorithm")
        .agg(pl.col("relative_to_a").log().mean().exp())
        .sort("algorithm")
        .collect()
    )
    print("Fastest candidate per workload:")
    print(
        medians.sort("ns")
        .group_by(keys)
        .agg(pl.col("algorithm").first())
        .group_by("algorithm")
        .len()
        .sort("algorithm")
        .collect()
    )
    print("1M positive weights (milliseconds and bytes):")
    print(
        medians.filter((pl.col("n") == 1_000_000) & (pl.col("weights") == "positive"))
        .with_columns(
            pl.col("ns", "preprocessing_ns", "optimization_ns", "reconstruction_ns") / 1e6,
        )
        .sort("family", "order", "algorithm")
        .collect()
    )
    print("B relative to A by structure (geometric mean):")
    print(
        compared.filter(pl.col("algorithm") == "B_two_orders")
        .group_by("family")
        .agg(pl.col("relative_to_a").log().mean().exp())
        .sort("family")
        .collect()
    )


if __name__ == "__main__":
    with pl.Config(tbl_rows=-1, tbl_cols=-1, tbl_width_chars=220, tbl_formatting="ASCII_FULL"):
        main()
