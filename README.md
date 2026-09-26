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
non-null integers, `Date`, or `Datetime` with matching dtypes and `start <= end`.

## Date and Datetime intervals

```python
from datetime import datetime
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [
            datetime(2026, 1, 1, 9, 0),
            datetime(2026, 1, 1, 9, 30),
            datetime(2026, 1, 1, 10, 0),
        ],
        "end": [
            datetime(2026, 1, 1, 10, 0),
            datetime(2026, 1, 1, 10, 30),
            datetime(2026, 1, 1, 11, 0),
        ],
    }
)

result = df.with_columns(pi.overlap_count("start", "end").alias("overlaps"))
print(result["overlaps"].to_list())  # [1, 2, 1]
```

The first and last appointments touch at 10:00; the middle one overlaps both.
Python `date` values work too, as `pl.Date` columns.

Both endpoints must be `Date`, or both must be `Datetime` with exactly matching
time units (`ms`, `us`, or `ns`) and timezone metadata. Matching timezone-aware
columns are supported. Date/Datetime, temporal/integer, different units, and
different timezones (including naive/aware pairs) are rejected without coercion.
`Time` and `Duration` are unsupported. Temporal columns use their physical integer
days or timestamps in the existing Rust algorithm, preserving exact values.

The API is early-stage and may change.

## Contributing

See the [contributing guide](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md)
for source builds and development checks. Licensed under [MIT](https://github.com/jplauri/polars-intervals/blob/master/LICENSE).
