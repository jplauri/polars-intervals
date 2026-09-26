"""Summarize recorded release results: python benchmarks/stabbing_summary.py."""

from pathlib import Path

import polars as pl


def main():
    root = Path(__file__).parent / "results"
    keys = ["dtype", "family", "order", "n"]
    medians = (
        pl.scan_csv(root / "stabbing-windows.csv")
        .group_by(*keys, "method")
        .agg((pl.col("total_ns").median() / 1e6).alias("ms"), pl.col("peak_bytes").first())
        .collect()
    )
    comparison = medians.pivot(on="method", index=keys, values="ms")
    pl.Config.set_tbl_rows(80)
    pl.Config.set_tbl_cols(12)
    pl.Config.set_tbl_formatting("ASCII_MARKDOWN")
    print("3M Int64 medians (milliseconds)")
    print(
        comparison.filter((pl.col("n") == 3_000_000) & (pl.col("dtype") == "i64")).sort(
            "family", "order"
        )
    )
    print("Geometric mean runtime ratios, per order (C/A below 1 favors detection)")
    print(
        comparison.lazy()
        .group_by("order")
        .agg(
            (pl.col("C") / pl.col("A")).log().mean().exp().alias("C/A"),
            (pl.col("B") / pl.col("A")).log().mean().exp().alias("B/A"),
        )
        .sort("order")
        .collect()
    )
    print("Detection overhead on unsorted input, excluding equal-end families")
    print(
        comparison.lazy()
        .filter(
            ~pl.col("family").is_in(["identical", "equal_ends"]) & (pl.col("order") != "sorted")
        )
        .group_by("order")
        .agg((pl.col("C") / pl.col("A")).log().mean().exp().alias("C/A"))
        .sort("order")
        .collect()
    )
    print("Production 3M requested peak heap bytes")
    print(
        medians.filter(
            (pl.col("method") == "production")
            & (pl.col("n") == 3_000_000)
            & (pl.col("dtype") == "i64")
        ).sort("family", "order")
    )
    temporal = root / "stabbing-temporal-windows.csv"
    if temporal.exists():
        print("Installed release wheel: 3M medians (milliseconds)")
        print(
            pl.scan_csv(temporal)
            .filter(pl.col("n") == 3_000_000)
            .group_by("dtype", "family", "order")
            .agg((pl.col("total_ns").median() / 1e6).alias("ms"))
            .sort("family", "order", "dtype")
            .collect()
        )


if __name__ == "__main__":
    main()
