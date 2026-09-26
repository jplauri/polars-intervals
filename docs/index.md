# Getting started

polars-intervals counts overlaps and assigns intervals to the minimum number of
non-overlapping lanes in Polars. Both operations return one value per row and
work with eager and lazy queries.

## Install

```sh
pip install polars-intervals
```

For a uv project, use `uv add polars-intervals`.
Requires Python 3.12+ and Polars `>=1.44.1,<1.45`.
For source builds, see [Contributing](contributing.md#build-from-source).

## Your first query

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count("start", "end").alias("overlaps"),
    )
    .collect()
)
print(result["overlaps"].to_list())
```

```text
[1, 1, 2, 0]
```

Each count excludes the row itself. Endpoints can be matching integer, Date,
or Datetime columns (see [input rules](usage.md#inputs)). Intervals
include their start but exclude their end: `[1, 3)` and `[3, 5)` do not overlap.
The empty interval `[2, 2)` counts zero.

## Next steps

- [Usage](usage.md): assign lanes, count within groups, and choose which rows to compare.
- [API reference](api.md): input requirements, return types, and errors.
- [Benchmarks](benchmarks.md): measured results and reproduction commands.
- [Release notes](changelog.md): changes and compatibility by version.

The API is early-stage and may change.
