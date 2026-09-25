# polars-intervals

Count overlapping intervals in Polars, without building a table of overlapping pairs.

[Usage](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md) ·
[Benchmarks](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md) ·
[Releases](https://github.com/jplauri/polars-intervals/releases)

## Install

```sh
pip install polars-intervals
```

Or with uv: `uv add polars-intervals`.

Requires Python 3.12+ and Polars `>=1.44.1,<1.45`.

## Count overlaps

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
print(result)
```

```text
shape: (4, 3)
┌───────┬─────┬──────────┐
│ start ┆ end ┆ overlaps │
│ ---   ┆ --- ┆ ---      │
│ i64   ┆ i64 ┆ u64      │
╞═══════╪═════╪══════════╡
│ 1     ┆ 3   ┆ 1        │
│ 3     ┆ 5   ┆ 1        │
│ 2     ┆ 4   ┆ 2        │
│ 2     ┆ 2   ┆ 0        │
└───────┴─────┴──────────┘
```

`overlap_count` works in eager and lazy queries. Add `.over("group")` to count
within each group.

Intervals are half-open: `[start, end)`. Touching intervals do not overlap,
empty intervals count zero, and each row excludes itself. Endpoints must be
non-null integers with matching dtypes and `start <= end`.

The API is early-stage and may change.

## Contributing

See the [contributing guide](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md)
for source builds and development checks. Licensed under [MIT](https://github.com/jplauri/polars-intervals/blob/master/LICENSE).
