"""End-to-end release-wheel benchmark; run after building/installing a release wheel.

No native expression baseline: these are iterative subset selection operations.
Times include plugin dispatch, validation, physical adaptation and mask construction.
"""

import csv
import random
import sys
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi


def main():
    writer = csv.writer(sys.stdout, lineterminator="\n")
    writer.writerow(["family", "order", "dtype", "n", "method", "sample", "total_ns", "selected"])
    rng = random.Random(20260926)
    for n in (1_000, 10_000, 100_000, 1_000_000):
        for family in ("chain", "dense", "duplicates", "irrelevant"):
            if family == "chain":
                rows = [(i, i + 1, 1) for i in range(n)]
            elif family == "dense":
                rows = [(max(0, i - n // 4), min(n, i + n // 4), i % 21) for i in range(n)]
            elif family == "duplicates":
                rows = [(0, n, i % 21) for i in range(n)]
            else:
                rows = [(n + i + 1, n + i + 2, 1) for i in range(n)]
                rows[0] = (0, n, 1)
            for order in ("sorted", "shuffled"):
                if order == "shuffled":
                    rng.shuffle(rows)
                base = pl.DataFrame(rows, schema=["start", "end", "cost"], orient="row")
                for dtype in (
                    pl.Int32,
                    pl.Int64,
                    pl.UInt64,
                    pl.Date,
                    pl.Datetime("ms"),
                    pl.Datetime("us"),
                    pl.Datetime("ns", "UTC"),
                    pl.Datetime("ns", "Europe/Helsinki"),
                ):
                    frame = base.lazy().with_columns(pl.col("start", "end").cast(dtype)).collect()
                    left = pl.Series([0]).cast(dtype)
                    right = pl.Series([n]).cast(dtype)
                    for method in ("minimum_cover", "minimum_cost_cover"):
                        function = getattr(pi, method)
                        kwargs = {"cost": "cost"} if method == "minimum_cost_cover" else {}
                        expr = function(
                            "start", "end", target_start=left, target_end=right, **kwargs
                        )
                        expected = frame.select(expr).to_series()
                        selected = expected.sum()
                        chosen = (
                            frame.filter(expected)
                            .lazy()
                            .select(pl.col("start", "end").to_physical())
                            .sort("start")
                            .collect()
                        )
                        frontier = 0
                        for start, end in chosen.iter_rows():
                            assert start <= frontier and start < end
                            frontier = max(frontier, end)
                        assert frontier >= n
                        for sample in range(3):
                            begin = perf_counter_ns()
                            result = frame.select(expr).to_series()
                            elapsed = perf_counter_ns() - begin
                            assert result.equals(expected)
                            writer.writerow(
                                [family, order, str(dtype), n, method, sample, elapsed, selected]
                            )


if __name__ == "__main__":
    main()
