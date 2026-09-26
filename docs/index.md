# Getting started

Interval algorithms for Polars, with support for eager and lazy queries.

## Install

Requires Python 3.12+.

```sh
pip install polars-intervals
```

Or with uv: `uv add polars-intervals`.

## Your first query

Count how many other intervals overlap each row:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 1, 2],
        "end": [2, 3, 4],
    }
)
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count("start", "end").alias("overlaps"),
    )
    .collect()
)

print(result["overlaps"].to_list())  # [1, 2, 1]
```

Intervals include their start and exclude their end, so touching intervals do not overlap.

## Next steps

- [Usage](usage.md): examples for each operation.
- [API reference](api.md): parameters, return types, and input requirements.
- [Benchmarks](benchmarks.md): comparisons and plots.
- [Contributing](contributing.md): source builds and development.
