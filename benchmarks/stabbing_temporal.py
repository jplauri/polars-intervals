"""Measure the installed release wheel, from outside the checkout with python -I.

No native-Polars baseline: this is a sequential global greedy selection, without
a clean equivalent in a small number of ordinary dataframe expressions.
"""

import csv
import sys
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi


def main():
    writer = csv.writer(sys.stdout, lineterminator="\n")
    writer.writerow(["family", "order", "dtype", "n", "sample", "total_ns", "points"])
    for n in (1_000, 10_000, 100_000, 1_000_000, 3_000_000):
        for family in ("disjoint", "dense", "identical"):
            i = pl.int_range(0, n)
            width = max(1, n // 8)
            starts, ends, optimum = {
                "disjoint": (i * 3, i * 3 + 2, n),
                "dense": (i, i + width, (n + width - 1) // width),
                "identical": (i * 0, i * 0 + 5, 1),
            }[family]
            base = pl.select(starts.alias("start"), ends.alias("end"))
            for order in ("sorted", "shuffled"):
                ordered = (
                    base
                    if order == "sorted"
                    else base.sample(fraction=1, shuffle=True, seed=20260926)
                )
                for dtype in (pl.Int64, pl.Date, pl.Datetime("us")):
                    frame = ordered.lazy().select(pl.all().cast(dtype)).collect()
                    expr = pi.minimum_stabbing_points("start", "end").alias("points")
                    query = frame.lazy().select(expr)
                    expected = query.collect()
                    assert expected.schema["points"] == pl.List(dtype)
                    points = expected.to_series().item().to_physical().cast(pl.Int64)
                    assert len(points) == optimum
                    assert points.is_sorted() and points.n_unique() == len(points)
                    # Every interval must contain its first point at/after start.
                    hit = (
                        base.lazy()
                        .join_asof(
                            pl.DataFrame({"point": points}).lazy(),
                            left_on="start",
                            right_on="point",
                            strategy="forward",
                        )
                        .select(
                            (
                                (pl.col("point") >= pl.col("start"))
                                & (pl.col("point") < pl.col("end"))
                            )
                            .fill_null(False)
                            .all()
                        )
                        .collect()
                        .item()
                    )
                    assert hit
                    # The known optimum is certified by n disjoint intervals, a
                    # width-spaced packing, or one non-empty interval respectively.
                    for sample in range(3):
                        begin = perf_counter_ns()
                        result = query.collect()
                        elapsed = perf_counter_ns() - begin
                        assert result.equals(expected)
                        writer.writerow(
                            [family, order, str(dtype), n, sample, elapsed, len(points)]
                        )


if __name__ == "__main__":
    main()
