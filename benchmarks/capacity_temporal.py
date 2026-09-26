"""Release-plugin timing on integer/temporal endpoints with an exact top-k oracle.

Run after `uv sync --locked`: uv run --locked python benchmarks/capacity_temporal.py
"""

import csv
import sys
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi


def main():
    output = csv.writer(sys.stdout, lineterminator="\n")
    output.writerow(["n", "k", "dtype", "sample", "ns", "objective"])
    for n in (1_000, 10_000, 100_000, 1_000_000):
        # Independent cliques of 32 rows, with weights 1..32 and a possible
        # shorter final clique. Endpoint casts and data construction are untimed.
        source = (
            pl.LazyFrame({"i": range(n)})
            .select(start=pl.col("i") // 32 * 100, weight=pl.col("i") % 32 + 1)
            .with_columns(end=pl.col("start") + 10)
            .collect()
        )
        for k in (0, 1, 2, 4, 8, 16, 31, 32, 33, 64):
            complete, remainder = divmod(n, 32)
            expected = complete * sum(range(33 - min(k, 32), 33)) + sum(
                range(remainder + 1 - min(k, remainder), remainder + 1)
            )
            for dtype in (pl.Int64, pl.Date, pl.Datetime("us"), pl.Datetime("ns", "UTC")):
                frame = source.lazy().with_columns(pl.col("start", "end").cast(dtype)).collect()
                query = frame.lazy().select(
                    pi.max_weight_with_capacity("start", "end", weight="weight", capacity=k)
                )
                for sample in range(-1, 3):
                    begin = perf_counter_ns()
                    mask = query.collect().to_series()
                    elapsed = perf_counter_ns() - begin
                    chosen = source.lazy().filter(mask)
                    objective, peak = (
                        chosen.select(
                            pl.col("weight").sum(),
                            pl.col("start").value_counts().struct.field("count").max(),
                        )
                        .collect()
                        .row(0)
                    )
                    assert objective == expected
                    assert (peak or 0) <= k
                    if sample >= 0:
                        output.writerow([n, k, str(dtype), sample, elapsed, objective])


if __name__ == "__main__":
    main()
