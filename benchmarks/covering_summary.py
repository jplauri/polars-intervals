"""Summarize release candidate measurements without weighting repeated samples twice."""

import sys
from pathlib import Path

import polars as pl

path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("benchmarks/results/covering-windows.csv")
keys = ["family", "order", "costs", "n"]
medians = (
    pl.scan_csv(path)
    .group_by(*keys, "method")
    .agg(pl.col("total_ns").median(), pl.col("peak_bytes").first())
)
baseline = medians.filter(pl.col("method").is_in(["MC-A", "MCC-A"])).select(
    *keys,
    pl.col("method").str.starts_with("MCC").alias("weighted"),
    pl.col("total_ns").alias("baseline_ns"),
)
ratios = (
    medians.with_columns(pl.col("method").str.starts_with("MCC").alias("weighted"))
    .join(baseline, on=[*keys, "weighted"])
    .with_columns((pl.col("total_ns") / pl.col("baseline_ns")).alias("ratio"))
)
print(
    ratios.group_by("method", "order")
    .agg(pl.col("ratio").median().round(3), (pl.col("ratio") < 1).sum().alias("wins"), pl.len())
    .sort("method", "order")
    .collect()
    .write_csv()
)
print(
    medians.filter(
        (pl.col("n") == 1_000_000) & (pl.col("order") == "shuffled") & (pl.col("costs") == "random")
    )
    .with_columns(
        (pl.col("total_ns") / 1e6).round(3).alias("ms"),
        (pl.col("peak_bytes") / 1e6).round(2).alias("MB"),
    )
    .select("family", "method", "ms", "MB")
    .sort("family", "method")
    .collect()
    .write_csv()
)
