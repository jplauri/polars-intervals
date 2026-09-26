"""Print reproducible per-case median timing/peak allocation tables from raw CSVs.

uv run --locked python benchmarks/capacity_summary.py benchmarks/results/capacity-windows.csv
"""

import sys

import polars as pl


def main():
    paths = sys.argv[1:] or ["benchmarks/results/capacity-windows.csv"]
    source = pl.concat(
        pl.scan_csv(path, schema_overrides={"objective": pl.String}) for path in paths
    )
    methods = [
        "production",
        "whole",
        "whole_guarded",
        "components",
        "parallel",
        "generic",
        "capacity_one",
    ]
    keys = ["n", "family", "order", "weights", "k", "concurrency", "components"]
    result = (
        source.group_by(keys)
        .agg(
            *(
                (pl.col("ns").filter(pl.col("algorithm") == method).median() / 1_000_000).alias(
                    f"{method}_ms"
                )
                for method in methods
            ),
            *(
                (
                    pl.col("peak_allocated_bytes").filter(pl.col("algorithm") == method).max()
                    / 2**20
                ).alias(f"{method}_MiB")
                for method in methods
            ),
        )
        .sort(keys)
        .collect()
    )
    sys.stdout.write(result.write_csv())


if __name__ == "__main__":
    main()
