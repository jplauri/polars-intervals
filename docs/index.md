# polars-intervals

Interval algorithms for Polars, implemented in Rust with support for eager and
lazy queries. Count overlaps and containment, measure nesting, assign lanes,
and select intervals for scheduling, covering, and coverage problems.

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

- [Usage](usage.md): input rules and examples for each operation.
- [API reference](api.md): parameters, return types, and input requirements.
- [Benchmarks](benchmarks.md): runtimes and native Polars comparisons.
- [Contributing](contributing.md): source builds and development.
