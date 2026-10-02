# Usage

Use polars-intervals to compare intervals, assign lanes, and choose intervals
for scheduling or covering. You can also combine intervals, find gaps, and
measure coverage or resource demand.

## A first example

Suppose three appointments run from 0 to 2, 1 to 3, and 2 to 4. Count how many
other appointments overlap each one:

```python
import polars as pl
import polars_intervals as pi

appointments = pl.DataFrame({"start": [0, 1, 2], "end": [2, 3, 4]})
result = (
    appointments.lazy().with_columns(pi.overlap_count("start", "end").alias("overlaps")).collect()
)

print(result["overlaps"].to_list())  # [1, 2, 1]
```

The middle appointment overlaps both neighbors. The first and last only touch
at 2, so they do not overlap. The remaining examples use the same imports.

## Inputs

### Interval rules

Each row describes a half-open interval `[start, end)`: it includes its start
and excludes its end. An interval with `start == end` is empty. Empty intervals
overlap nothing.

Endpoints must be non-null, with `start <= end` in every row. Both columns must
have the same dtype, and nothing is cast for you:

| Endpoint type | Requirement |
| --- | --- |
| Integer | The same signed or unsigned 8-, 16-, 32-, or 64-bit dtype |
| `Date` | Both columns must be `Date` |
| `Datetime` | The same time unit and timezone |

Other endpoint types, including `Int128`, floating-point, Boolean, Decimal,
`Time` and `Duration`, are rejected.

### Weights, costs, and other values

Cost, weight, capacity and initial-lane columns accept `Int8`, `Int16`, `Int32`,
`Int64`, `UInt8`, `UInt16`, `UInt32` and `UInt64`. Supply one non-null value per
input row. Values are not cast automatically. All other dtypes are rejected,
including floating-point, Boolean, Decimal, temporal and 128-bit integers.

| Input | Functions | Allowed values |
| --- | --- | --- |
| Selection `weight` | [`max_weight_non_overlapping`](#select-a-globally-maximum-weight-schedule), [`max_weight_with_capacity`](#select-with-a-simultaneous-capacity), [`max_weight_with_capacity_profile`](#select-with-a-capacity-profile), [`max_weight_clique`](#select-a-maximum-weight-clique) | Zero and negative values are valid but never selected. |
| `cost` | [`minimum_cost_cover`](#cover-at-minimum-cost), [`minimum_cost_dominating_set`](#select-a-minimum-cost-dominating-set) | Nonnegative. Zero is valid. |
| Quantity `weight` | [`coverage_profile`](coverage-profile.md) | Nonnegative. Zero is valid. |
| Profile `capacity` | [`max_weight_with_capacity_profile`](#select-with-a-capacity-profile) | Nonnegative. Zero is valid. |
| `initial_lanes` | [`assign_balanced_lanes`](#balance-lane-row-counts) | IDs must fit `UInt32` and form a valid assignment using the minimum number of lanes, with contiguous IDs from zero. |

Omitted costs in `minimum_cost_dominating_set` and omitted weights in
`max_weight_clique` or `coverage_profile` mean one per row. These functions do
not automatically use a column named `cost`, `weight` or `load`.
`max_weight_with_capacity_profile` reads the `weight` and `capacity` columns by
default.

Scalar options `capacity`, `k` and `max_work` require nonnegative Python integers.
Boolean values are rejected. See each function's [API reference](api.md) for its
range limits.

### Column names, expressions, and results

Most functions accept column names such as `"start"` or expressions such as
`pl.col("start")`, and return an expression for an eager or lazy query. The
input does not need to be sorted.

Selections return a Boolean mask. Use it in `filter` to keep the chosen rows,
or in `with_columns` to flag them.

### Frame results

[`coverage_stats`](coverage-stats.md) appends source counts, exact covered and
query lengths, and covered fractions to every query row. It preserves query
payloads and global row order. Pass two DataFrames for an immediate result.
If either input is lazy, both inputs participate in one deferred calculation.
Use `by` for shared keys, including null-key matches.

`coverage_profile`, `merge_intervals` and `interval_gaps` return new segment
rows. Pass a DataFrame for an immediate result or a LazyFrame for a query that
runs when collected. The result has the same frame kind as the input. Use these
functions directly or with `.pipe(...)`.

[`subtract_intervals` and `intersect_intervals`](interval-geometry.md#subtract-and-intersect-two-collections)
take two frames and return new covered ranges. Two DataFrames return a DataFrame.
If either input is lazy, both inputs participate in one deferred query. All four
endpoint columns must have exactly the same logical dtype. Shared group keys
must also match in dtype. Result groups follow the left input's first appearances.

Endpoint arguments take literal column names. Output endpoints are always
named `start` and `end`. Endpoint and group-key dtypes are preserved, including
in empty results. Other input columns are not retained.

Use `by="resource"` or an ordered list of distinct column names to solve groups
independently. `by=None` and `by=[]` use one collection. Keys support String,
Boolean, the integer types above, Date and Datetime. Null key values group
together. Group keys cannot use names reserved for output columns.

Groups appear in first-appearance order, with segments sorted by start within
each group. Only groups present in the input can produce output. A grouped
empty frame has no output groups.

These functions need their complete input in memory, even with
`.collect(engine="streaming")`. All chunks participate in the same operation.
Schema checks happen when the query is built. Row validation happens when it
runs and covers every input row, including empty or clipped-away intervals.

### Count within groups

Use `.over(...)` to solve each group independently while keeping row order:

```python
appointments = pl.DataFrame(
    {
        "room": ["a", "b", "a", "b"],
        "start": [1, 1, 2, 5],
        "end": [4, 4, 3, 6],
    }
)
result = (
    appointments.lazy()
    .with_columns(pi.overlap_count("start", "end").over("room").alias("overlaps"))
    .collect()
)

print(result["overlaps"].to_list())  # [1, 0, 1, 0]
```

This works for every per-row result and selection mask. Use
`group_by(...).agg(...)` instead to get one list per group.

### Choose which rows to compare

Filtering before an operation changes the intervals it sees. To keep a result
computed from the full input, add it with `with_columns` first, then filter.

## Choose an algorithm

| Task | Algorithms |
| --- | --- |
| [Inspect interval relationships](#inspect-interval-relationships) | Count overlaps, count contained rows, measure nesting depth |
| [Interval geometry](interval-geometry.md) | Label connected rows, combine ranges, find gaps, subtract coverage, or intersect collections |
| [Assign lanes](#assign-lanes) | Use the fewest lanes, then optionally balance their row counts |
| [Select intervals by weight](#select-intervals-by-weight) | Choose a schedule with one or more slots, or choose a mutually overlapping set |
| [Covering and coverage](#covering-and-coverage) | Cover a target, maximize covered length with a budget, or hit every interval with points |
| [Choose representative intervals](#choose-representative-intervals) | Select rows so every input row is selected or overlaps a selected row |
| [Coverage and load profiles](coverage-profile.md) | Measure how much is active at each coordinate |

The [API reference](api.md) has full signatures and edge cases.

## Inspect interval relationships

### Count overlaps

`overlap_count` counts how many other intervals overlap each row, as in the
[first example](#a-first-example).

[API reference](api.md#polars_intervals.overlap_count) |
[Benchmarks](overlap-count-benchmarks.md)

### Count containment

`containment_count` counts the other intervals **inside** each row:

```python
intervals = pl.DataFrame({"start": [0, 2, 4], "end": [10, 5, 12]})
result = (
    intervals.lazy().with_columns(pi.containment_count("start", "end").alias("contained")).collect()
)

print(result["contained"].to_list())  # [1, 0, 0]
```

The first interval contains `[2, 5)`. It overlaps `[4, 12)`, but does not
contain it. Identical rows contain one another.

[API reference](api.md#polars_intervals.containment_count) |
[Benchmarks](containment-benchmarks.md)

### Nesting depth

`nesting_depth` counts how many levels of containers sit **above** each row.
Outermost intervals have depth zero:

```python
intervals = pl.DataFrame({"start": [0, 2, 2, 3], "end": [10, 8, 8, 7]})
result = intervals.lazy().with_columns(pi.nesting_depth("start", "end").alias("depth")).collect()

print(result["depth"].to_list())  # [0, 1, 1, 2]
```

Identical intervals share a depth, like the two `[2, 8)` rows. Depth follows
the longest chain, not the number of containers: `[0, 8)` and `[2, 10)` both
contain `[4, 5)`, but neither contains the other, so `[4, 5)` has depth 1.

[API reference](api.md#polars_intervals.nesting_depth) |
[Benchmarks and algorithm notes](nesting-depth-benchmarks.md)

## Assign lanes

Use lane assignment when every interval needs a place, such as a room in a
calendar or a track in a timeline. Intervals in the same lane do not overlap.

### Assign the minimum number of lanes

`assign_lanes` places all intervals into the fewest possible lanes:

```python
appointments = pl.DataFrame({"start": [0, 1, 2], "end": [2, 3, 4]})
result = appointments.lazy().with_columns(pi.assign_lanes("start", "end").alias("lane")).collect()

print(result["lane"].to_list())  # [0, 1, 0]
```

The first and last appointments touch, so they share a lane.

[API reference](api.md#polars_intervals.assign_lanes) |
[Benchmarks](assign-lanes-benchmarks.md)

### Balance lane row counts

`assign_balanced_lanes` also uses the fewest lanes, then tries to even out the
**number of rows** in each:

```python
intervals = pl.DataFrame({"start": [0, 0, 1, 2, 3, 4], "end": [10, 10, 1, 2, 3, 4]})
result = (
    intervals.lazy().with_columns(pi.assign_balanced_lanes("start", "end").alias("lane")).collect()
)

print(sorted(result["lane"].value_counts()["count"].to_list()))  # [3, 3]
```

The two long intervals need two lanes, and the four empty rows are split
between them. Balancing is a heuristic, so the most even split is not
guaranteed.

[API reference](api.md#polars_intervals.assign_balanced_lanes) |
[Measured quality, runtime, and work limits](balance-lanes-benchmarks.md)

## Select intervals by weight

These operations choose the rows with the largest total weight, such as revenue
or priority, under a constraint. Rows with zero or negative weight are never
selected.

### Select a globally maximum-weight schedule

`max_weight_non_overlapping` selects intervals that can run without overlaps:

```python
jobs = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "revenue": [15, 10, 10, 10],
    }
)
chosen = (
    jobs.lazy().filter(pi.max_weight_non_overlapping("start", "end", weight="revenue")).collect()
)

print(chosen["revenue"].sum())  # 30
```

The three shorter jobs earn 30 together, compared with 15 for the long job.

[API reference](api.md#polars_intervals.max_weight_non_overlapping) |
[Benchmarks](weighted-scheduling-benchmarks.md)

### Select with a simultaneous capacity

`max_weight_with_capacity` allows up to `capacity` selected intervals to
overlap. Using the same jobs:

```python
chosen = (
    jobs.lazy()
    .filter(pi.max_weight_with_capacity("start", "end", weight="revenue", capacity=2))
    .collect()
)

print(chosen["revenue"].sum())  # 45
```

With two slots, the long job can run alongside all three short jobs.

[API reference](api.md#polars_intervals.max_weight_with_capacity) |
[Benchmarks](capacity-scheduling-benchmarks.md)

### Select with a capacity profile

`max_weight_with_capacity_profile` handles capacity that changes over time.
Give it the jobs and a separate table describing the available slots:

```python
jobs = pl.DataFrame(
    {"start": [9, 10, 11, 14], "end": [13, 12, 15, 17], "weight": [100, 60, 130, 90]}
)
profile = pl.DataFrame({"start": [9, 12, 14], "end": [12, 14, 18], "capacity": [3, 1, 4]})
selected = pi.max_weight_with_capacity_profile(jobs, profile)
result = jobs.filter(selected)

print(selected.to_list())  # [False, True, True, True]
print(result["weight"].sum())  # 280
```

Capacity is three from 9 to 12, one from 12 to 14, and four from 14 to 18.
The jobs worth 100 and 130 both cross the noon bottleneck, so only one can be
selected. Times outside the profile have zero capacity.

[API reference](api.md#polars_intervals.max_weight_with_capacity_profile) |
[Benchmarks and algorithm notes](capacity-profile-benchmarks.md)

### Select a maximum-weight clique

Use `max_weight_clique` to choose intervals that **all overlap one another**.
Without `weight`, it selects as many rows as possible:

```python
intervals = pl.DataFrame({"start": [0, 1, 2, 10], "end": [5, 4, 3, 11], "value": [1, 1, 1, 9]})
most_rows = intervals.lazy().filter(pi.max_weight_clique("start", "end")).collect()
most_value = intervals.lazy().filter(pi.max_weight_clique("start", "end", weight="value")).collect()

print(most_rows["start"].to_list())  # [0, 1, 2]
print(most_value["start"].to_list())  # [10]
```

The first three rows form the largest clique, but the last row alone has
greater weight.

[API reference](api.md#polars_intervals.max_weight_clique) |
[Benchmarks and correctness notes](clique-benchmarks.md)

## Covering and coverage

### Cover one continuous target

`minimum_cover` selects the fewest intervals needed to cover a target:

```python
intervals = pl.DataFrame({"start": [0, 0, 4, 6, 7], "end": [4, 6, 7, 10, 10]})
chosen = (
    intervals.lazy()
    .filter(pi.minimum_cover("start", "end", target_start=0, target_end=10))
    .collect()
)

print(chosen.rows())  # [(0, 6), (6, 10)]
```

If the target cannot be fully covered, the query raises an error.

[API reference](api.md#polars_intervals.minimum_cover) |
[Benchmarks](covering-benchmarks.md)

### Cover at minimum cost

`minimum_cost_cover` covers the same kind of target at the lowest total cost:

```python
intervals = pl.DataFrame({"start": [0, 0, 5], "end": [10, 5, 10], "cost": [100, 10, 10]})
chosen = (
    intervals.lazy()
    .filter(pi.minimum_cost_cover("start", "end", cost="cost", target_start=0, target_end=10))
    .collect()
)

print(chosen.rows())  # [(0, 5, 10), (5, 10, 10)]
print(chosen["cost"].sum())  # 20
```

The two shorter intervals cost 20 together, compared with 100 for the single
full-length interval.

[API reference](api.md#polars_intervals.minimum_cost_cover) |
[Benchmarks](cost-covering-benchmarks.md)

### Select maximum coverage with a budget

`max_k_coverage` selects at most `k` intervals whose union has the greatest
total length:

```python
intervals = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
chosen = intervals.lazy().filter(pi.max_k_coverage("start", "end", k=2)).collect()

print(chosen.rows())  # [(-5, 4), (6, 15)]
```

The selected intervals cover 18 units. Starting from the longest interval,
`[0, 10)`, would cover at most 15.

[API reference](api.md#polars_intervals.max_k_coverage) |
[Benchmarks and algorithm notes](coverage-benchmarks.md)

### Minimum stabbing points

`minimum_stabbing_points` chooses the fewest points needed to hit every
interval:

```python
intervals = pl.DataFrame({"start": [0, 2, 5], "end": [4, 6, 9]})
result = (
    intervals.lazy().select(pi.minimum_stabbing_points("start", "end").alias("points")).collect()
)

print(result["points"].to_list())  # [[3, 8]]
```

Point 3 hits the first two intervals, and point 8 hits the third. The result is
one sorted list. Empty intervals raise an error because no point can hit them.

[API reference](api.md#polars_intervals.minimum_stabbing_points) |
[Benchmarks](stabbing-benchmarks.md)

## Choose representative intervals

Use representatives when every input interval must be selected or overlap a
selected interval.

### Select a minimum-cost dominating set

`minimum_cost_dominating_set` chooses representatives at minimum total cost.
Without `cost`, it selects the fewest rows:

```python
intervals = pl.DataFrame({"start": [0, 3, 6], "end": [4, 7, 10], "price": [1, 10, 1]})
fewest_rows = intervals.lazy().filter(pi.minimum_cost_dominating_set("start", "end")).collect()
lowest_cost = (
    intervals.lazy().filter(pi.minimum_cost_dominating_set("start", "end", cost="price")).collect()
)

print(fewest_rows["start"].to_list())  # [3]
print(lowest_cost["start"].to_list())  # [0, 6]
print(lowest_cost["price"].sum())  # 2
```

The middle interval overlaps both neighbors, so it alone represents all three.
With prices, the two outer intervals cost 2, compared with 10 for the middle.

[API reference](api.md#polars_intervals.minimum_cost_dominating_set) |
[Benchmarks and correctness notes](domination-benchmarks.md)
