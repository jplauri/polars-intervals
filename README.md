# polars-intervals

Count overlaps, assign optimal interval lanes, and select maximum-weight schedules
in Polars, without building a graph or a table of overlapping pairs.

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

## Assign lanes

`assign_lanes` assigns intervals to the minimum number of non-overlapping lanes.
Use it for calendar/timeline layout, machine/resource lanes, Gantt charts, genomic
tracks, and concurrent-job visualization.

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 1, 2], "end": [2, 3, 4]})
result = df.lazy().with_columns(pi.assign_lanes("start", "end").alias("lane")).collect()
assert result["lane"].dtype == pl.UInt32
assert result["lane"].n_unique() == 2
```

For non-empty intervals, **minimum number of lanes = maximum concurrency**.
Touching intervals can share a lane. Empty intervals consume no capacity and
receive lane `0`; a nonempty collection of only empty intervals uses one lane.
Empty input returns empty output. Lane IDs are contiguous `0..k-1`, deterministic
for identical input, and returned in original row order. No particular optimal
coloring is promised across releases or permutations of the rows.

Use `.over("group")` for independent lane assignment per group, or
`group_by(...).agg(...)` for lists. See the
[algorithm comparison](https://github.com/jplauri/polars-intervals/blob/master/docs/assign-lanes-benchmarks.md)
for the measured production choice.

## Select a maximum-weight schedule

`max_weight_non_overlapping` selects a globally maximum-weight subset of mutually
non-overlapping intervals. It returns a Boolean expression for filtering:

```python
df = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "weight": [15, 10, 10, 10],
    }
)
selected = df.filter(pi.max_weight_non_overlapping("start", "end", weight="weight"))
assert selected["weight"].sum() == 30
```

The three shorter intervals beat the single largest-weight interval (15).
This is an exact global optimization. Touching endpoints are compatible, and
every positive empty interval is selected. The empty subset is allowed;
negative and zero-weight rows are omitted. Weights must be non-null signed or
unsigned integers up to 64 bits, accumulated exactly with checked `i128` arithmetic.
Float and Decimal weights are unsupported. The mask follows original row order
and is deterministic for identical input; no particular optimum on ties is promised.
Use `.over("group")` for independent schedules or `group_by(...).agg(...)` for lists.

Time is `O(n log n)` and additional space is `O(n)`. See the
[weighted scheduling comparison](https://github.com/jplauri/polars-intervals/blob/master/docs/weighted-scheduling-benchmarks.md)
for reproducible candidate benchmarks and the production choice.

## Date and Datetime intervals

All three operations accept Date and Datetime endpoints.

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
