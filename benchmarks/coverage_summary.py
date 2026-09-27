"""Summarize recorded release measurements (no benchmarks are executed)."""

import sys
from pathlib import Path

import polars as pl


def main():
    path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(__file__).parent / "results/coverage-core.csv"
    )
    source = pl.scan_csv(path)
    keys = ["family", "order", "n", "k", "method"]
    metrics = [
        "total_ns",
        "sort_ns",
        "preprocess_ns",
        "dp_ns",
        "reconstruct_ns",
        "peak_bytes",
        "allocations",
    ]
    medians = source.group_by(keys).agg(pl.col(metrics).median(), pl.len().alias("samples"))
    frame = medians.sort(keys).collect()
    assert frame["samples"].min() == frame["samples"].max() == 3
    sys.stdout.write(frame.write_csv())


if __name__ == "__main__":
    main()
