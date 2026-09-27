"""Installed release-plugin timings; run with python -I outside the checkout."""

import csv
import sys
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi


def main():
    writer = csv.writer(sys.stdout, lineterminator="\n")
    writer.writerow(
        ["family", "order", "dtype", "n", "k", "sample", "total_ns", "measure", "count"]
    )
    for n in (1_000, 10_000, 100_000, 1_000_000):
        for family in ("disjoint", "staircase", "identical"):
            i = pl.int_range(0, n)
            starts, ends = {
                "disjoint": (i * 10, i * 10 + 7),
                "staircase": (i * 3, i * 3 + 17),
                "identical": (i * 0, i * 0 + 7),
            }[family]
            base = pl.select(starts.alias("start"), ends.alias("end"))
            for order in ("sorted", "shuffled"):
                ordered = (
                    base
                    if order == "sorted"
                    else base.sample(fraction=1, shuffle=True, seed=20260927)
                )
                for dtype in (pl.Int64, pl.Date, pl.Datetime("us")):
                    frame = ordered.lazy().select(pl.all().cast(dtype)).collect()
                    for k in (0, 1, 2, 8, 32, 64):
                        query = frame.lazy().select(
                            pi.max_k_coverage("start", "end", k=k).alias("selected")
                        )
                        expected = query.collect().to_series()
                        selected = (
                            frame.lazy()
                            .filter(pl.lit(expected))
                            .select(pl.all().to_physical().cast(pl.Int64))
                            .sort("start")
                            .collect()
                        )
                        # Independent merge; disjoint selections of k intervals
                        # certify the staircase maximum because n >= 6*k.
                        measure, right = 0, None
                        for start, end in selected.iter_rows():
                            measure += max(
                                0, end - max(start, right if right is not None else start)
                            )
                            right = max(end, right if right is not None else end)
                        count = 0 if k == 0 else (1 if family == "identical" else k)
                        optimum = count * (17 if family == "staircase" else 7)
                        assert expected.dtype == pl.Boolean and expected.null_count() == 0
                        assert measure == optimum and len(selected) == count
                        for sample in range(3):
                            begin = perf_counter_ns()
                            result = query.collect().to_series()
                            elapsed = perf_counter_ns() - begin
                            assert result.equals(expected)
                            writer.writerow(
                                [family, order, str(dtype), n, k, sample, elapsed, measure, count]
                            )


if __name__ == "__main__":
    main()
