# Usage

Pass column names or Polars expressions to use interval operations in eager or
lazy queries. See the [API reference](api.md) for full parameter and return types.

## Count overlaps

`overlap_count` counts how many other intervals overlap each row.

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [1, 3, 2, 2],
        "end": [3, 5, 4, 2],
    }
)
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count("start", "end").alias("overlaps"),
    )
    .collect()
)

print(result["overlaps"].to_list())  # [1, 1, 2, 0]
```

### Interval rules

Each count excludes the row itself. Empty intervals count zero and contribute
no overlaps.

| Case | Overlap counts |
| --- | --- |
| `[1, 3)` and `[3, 5)` | Zero for both, as touching endpoints do not overlap |
| `[1, 4)` and `[3, 5)` | One for each |
| `[2, 2)` | Zero |
| Two rows containing `[1, 4)` | One for each, as duplicate rows count separately |
| A single interval | Zero |

### Count within groups

Use `.over(...)` to compare intervals within each group while keeping the rows
and their order:

```python
df = pl.DataFrame(
    {
        "group": ["a", "b", "a", "b"],
        "start": [1, 1, 2, 5],
        "end": [4, 4, 3, 6],
    }
)
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count("start", "end").over("group").alias("overlaps"),
    )
    .collect()
)

print(result["overlaps"].to_list())  # [1, 0, 1, 0]
```

To collect the counts into one list per group, use `group_by(...).agg(...)`:

```python
grouped = (
    df.lazy()
    .group_by("group", maintain_order=True)
    .agg(
        pi.overlap_count("start", "end").alias("overlaps"),
    )
    .collect()
)

print(grouped.rows())  # [('a', [1, 1]), ('b', [0, 0])]
```

### Choose which rows to compare

Filtering before counting compares only the retained intervals. Filtering
afterwards keeps counts computed against the full input:

```python
df = pl.DataFrame(
    {
        "start": [1, 3, 2, 2],
        "end": [3, 5, 4, 2],
    }
)
count = pi.overlap_count("start", "end").alias("overlaps")
keep = pl.col("start") < 3

within_subset = df.filter(keep).with_columns(count)
against_all = df.with_columns(count).filter(keep)

print(within_subset["overlaps"].to_list())  # [1, 1, 0]
print(against_all["overlaps"].to_list())  # [1, 2, 0]
```

The interval `[2, 4)` has one overlap in the subset and two in the full input.

## Assign the minimum number of lanes

`assign_lanes` places intervals into the fewest lanes without overlaps within
a lane. This is useful for calendars, timelines, and resource scheduling.

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
        pi.assign_lanes("start", "end").alias("lane"),
    )
    .collect()
)

print(result["lane"].to_list())  # [0, 1, 0]
```

The first and last intervals touch, so they share a lane. Empty intervals use
no capacity and receive lane `0`. Lane IDs start at zero, but their assignment
may change across releases or input order. Use `.over("group")` for separate
assignments within groups.

[API reference](api.md#polars_intervals.assign_lanes) · [Benchmarks](assign-lanes-benchmarks.md)

## Select a globally maximum-weight schedule

`max_weight_non_overlapping` selects non-overlapping intervals with the largest
total weight:

```python
import polars as pl
import polars_intervals as pi

jobs = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "revenue": [15, 10, 10, 10],
    }
)
chosen = jobs.filter(
    pi.max_weight_non_overlapping("start", "end", weight="revenue"),
)

print(chosen["revenue"].sum())  # 30
```

The three shorter jobs earn 30 together, compared with 15 for the long job.
Weights must be non-null integers up to 64 bits. Rows with zero or negative
weight are omitted. Positive-weight empty intervals are always selected.

[API reference](api.md#polars_intervals.max_weight_non_overlapping) · [Benchmarks](weighted-scheduling-benchmarks.md)

## Select with a simultaneous capacity

`max_weight_with_capacity` allows up to `capacity` selected intervals to overlap:

```python
import polars as pl
import polars_intervals as pi

jobs = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "revenue": [15, 10, 10, 10],
    }
)
chosen = jobs.filter(
    pi.max_weight_with_capacity(
        "start",
        "end",
        weight="revenue",
        capacity=2,
    )
)

print(chosen["revenue"].sum())  # 45
```

At capacity two, the long job can run alongside all three short jobs. Capacity
one gives the same optimum as `max_weight_non_overlapping`. The same weight
rules apply. Capacity must be a nonnegative integer, and empty intervals use
no capacity.

[API reference](api.md#polars_intervals.max_weight_with_capacity) · [Benchmarks](capacity-scheduling-benchmarks.md)

## Cover one continuous target

`minimum_cover` selects the fewest intervals needed to cover a target:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 0, 4, 6, 7],
        "end": [4, 6, 7, 10, 10],
    }
)
chosen = df.filter(
    pi.minimum_cover(
        "start",
        "end",
        target_start=0,
        target_end=10,
    )
)

print(chosen.rows())  # [(0, 6), (6, 10)]
```

Touching intervals can form a continuous cover. Intervals may extend beyond the
target. An empty target selects nothing, and an uncovered target raises an error.

[API reference](api.md#polars_intervals.minimum_cover) · [Benchmarks](covering-benchmarks.md)

### Cover at minimum cost

`minimum_cost_cover` minimizes total cost, choosing fewer intervals when costs tie:

```python
df = pl.DataFrame(
    {
        "start": [0, 0, 5],
        "end": [10, 5, 10],
        "cost": [100, 10, 10],
    }
)
chosen = df.filter(
    pi.minimum_cost_cover(
        "start",
        "end",
        cost="cost",
        target_start=0,
        target_end=10,
    )
)

print(chosen.rows())  # [(0, 5, 10), (5, 10, 10)]
print(chosen["cost"].sum())  # 20
```

The two shorter intervals cost 20 together, compared with 100 for the single
full-length interval. Costs must be non-null, nonnegative integers up to 64 bits.

For both covering operations, `.over("group")` covers the same target separately
within each group. An infeasible group raises an error.

[API reference](api.md#polars_intervals.minimum_cost_cover) · [Benchmarks](cost-covering-benchmarks.md)

### Date and Datetime targets

Python dates work with `Date` endpoints. Python datetimes require matching
microsecond `Datetime` columns and timezone metadata. To specify another time
unit, pass a one-element Series with the same dtype as the endpoints:

```python
target_start = pl.Series([0], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
target_end = pl.Series([100], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
```

See the [target requirements](api.md#polars_intervals.minimum_cover) for supported
timezones and scalar types.

## Minimum stabbing points

`minimum_stabbing_points` finds the fewest points needed to hit every interval:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 2, 5],
        "end": [4, 6, 9],
    }
)
result = df.select(
    pi.minimum_stabbing_points("start", "end").alias("points"),
)

print(result["points"].to_list())  # [[3, 8]]
```

Point 3 hits the first two intervals, and point 8 hits the third. The result is
one sorted list with the endpoint dtype. Empty input returns one empty list.
An empty interval raises an error because it contains no point.

Use grouped aggregation for one list per group:

```python
grouped = (
    df.with_columns(pl.Series("group", ["a", "a", "b"]))
    .group_by("group", maintain_order=True)
    .agg(
        pi.minimum_stabbing_points("start", "end").alias("points"),
    )
)

print(grouped.rows())  # [('a', [3]), ('b', [8])]
```

[API reference](api.md#polars_intervals.minimum_stabbing_points) · [Benchmarks](stabbing-benchmarks.md)

## Inputs

Intervals include their start and exclude their end: `[start, end)`.
Endpoints must be non-null and satisfy `start <= end`.

| Endpoint type | Requirement |
| --- | --- |
| Integer | Matching signed or unsigned integer dtypes up to 64 bits |
| `Date` | Both columns must be `Date` |
| `Datetime` | Matching time unit and timezone metadata |

Inputs are not cast automatically. Cast mismatched columns explicitly to a type
that can hold every endpoint. `Time`, `Duration`, and floating-point endpoints
are unsupported. See the [API reference](api.md) for validation details.

Each query or group is solved as a whole, including all input chunks. Collecting
with the streaming engine still requires the operation to see that collection.
