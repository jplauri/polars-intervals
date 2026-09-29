# Usage

Use polars-intervals to compare intervals, assign lanes, and choose intervals
for scheduling or covering. This guide starts with a small query, explains
the input rules, then walks through the available algorithms by task.

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
at 2, so they do not overlap. `with_columns` keeps the appointments and adds
one count per row. The remaining examples use the same `pl` and `pi` imports.

## Inputs

### Interval rules

Each row describes a half-open interval `[start, end)`: it includes its start
and excludes its end. This means `[0, 2)` and `[2, 4)` can share a lane or run
consecutively in a schedule. An interval with `start == end` is empty.

Start and end columns must have equal lengths, contain no nulls, and satisfy
`start <= end` in every row. Their dtypes must match exactly:

| Endpoint type | Requirement |
| --- | --- |
| Integer | The same signed or unsigned 8-, 16-, 32-, or 64-bit dtype |
| `Date` | Both columns must be `Date` |
| `Datetime` | The same time unit (`ms`, `us`, or `ns`) and timezone metadata |

Inputs are not cast, filled, or broadcast automatically. Cast mismatched
columns explicitly to a type that can hold every endpoint. Floating-point,
`Time`, and `Duration` endpoints are unsupported. These rules also apply to
empty inputs: give empty columns a supported dtype. Weight, cost, and
capacity-profile columns must likewise be non-null signed or unsigned integers
up to 64 bits, with one value per row.

Empty intervals overlap nothing and use no capacity. Each operation below
notes any other empty-interval behavior.

### Column names, expressions, and results

Most functions accept column names such as `"start"` or Polars expressions
such as `pl.col("start")`. They return an expression to use in an eager or
lazy query. You do not need to sort the input first.

| Result you need | Use the expression in | Returned values |
| --- | --- | --- |
| A count, depth, or lane for each row | `with_columns(expr.alias("name"))` | `UInt64` counts/depths or `UInt32` lane IDs |
| Only the chosen intervals | `filter(...)` | A Boolean mask keeps selected rows |
| A selection flag alongside every row | `with_columns(expr.alias("selected"))` | One Boolean per input row |
| Points that hit all intervals | `select(pi.minimum_stabbing_points(...))` | One list of coordinates |

Per-row results contain no nulls and stay aligned with the original rows.
Selection masks choose one optimal solution. When several solutions tie, the
particular rows chosen may change across releases or row permutations.

Two functions take frames directly. The
[capacity-profile selector](#select-with-a-capacity-profile) takes eager
DataFrames and returns a Boolean Series, so collect lazy inputs first.
[`coverage_profile`](coverage-profile.md) accepts a DataFrame or LazyFrame and
returns segments of coverage depth or resource load in the same frame type.

### Count within groups

By default, an expression compares or optimizes the whole input collection.
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

The same pattern works for containment, nesting, lanes, and selection masks.
Scalar options such as `k`, `capacity`, and covering targets apply separately
to every group. To return one list of per-row results per group, use
`group_by(...).agg(...)`:

```python
grouped = (
    appointments.lazy()
    .group_by("room", maintain_order=True)
    .agg(pi.overlap_count("start", "end").alias("overlaps"))
    .collect()
)

print(grouped.rows())  # [('a', [1, 1]), ('b', [0, 0])]
```

Each collection or group is solved as a whole across all input chunks.
Collecting with the streaming engine still requires the operation to see
that complete collection.

### Choose which rows to compare

Filtering before an operation changes the intervals it sees. Filtering after
computing a count or selection keeps the result calculated from the full input:

```python
intervals = pl.DataFrame({"start": [1, 3, 2], "end": [3, 5, 4]})
count = pi.overlap_count("start", "end").alias("overlaps")
keep = pl.col("start") < 3

within_subset = intervals.lazy().filter(keep).with_columns(count).collect()
against_all = intervals.lazy().with_columns(count).filter(keep).collect()

print(within_subset["overlaps"].to_list())  # [1, 1]
print(against_all["overlaps"].to_list())  # [1, 2]
```

For selection functions, compute a `selected` column first when you need to
filter an already chosen solution.

## Choose an algorithm

Start with the question you want to answer. Within each category, the simpler
operation comes first, followed by variants with additional constraints.

| Task | Algorithms |
| --- | --- |
| [Inspect interval relationships](#inspect-interval-relationships) | Count overlaps, count contained rows, measure nesting depth |
| [Assign lanes](#assign-lanes) | Use the fewest lanes, then optionally balance their row counts |
| [Select intervals by weight](#select-intervals-by-weight) | Choose a schedule with one or more slots, or choose a mutually overlapping set |
| [Covering and coverage](#covering-and-coverage) | Cover a target, maximize covered length with a budget, or hit every interval with points |
| [Choose representative intervals](#choose-representative-intervals) | Select rows so every input row is selected or overlaps a selected row |

See the [API reference](api.md) for full signatures and validation details.
Each example below also links to benchmarks and algorithm notes.

## Inspect interval relationships

These operations describe every input row. Use them to find conflicts,
contained intervals, or levels in a nested collection.

### Count overlaps

`overlap_count` answers: how many other intervals overlap this row? The
[first example](#a-first-example) shows the basic query. Each count excludes
the row itself.

| Input intervals | Overlap counts |
| --- | --- |
| `[1, 3)`, `[3, 5)` | `0, 0`: touching endpoints do not overlap |
| `[1, 4)`, `[3, 5)` | `1, 1` |
| `[1, 4)`, `[1, 4)` | `1, 1`: duplicates are separate rows |
| `[1, 4)`, `[2, 2)` | `0, 0`: empty intervals overlap nothing |

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
contain it. Containment requires both `A.start <= B.start` and `B.end <= A.end`,
with the row itself excluded.

These endpoint comparisons also apply to duplicates and empty intervals:

| Input intervals | Containment counts |
| --- | --- |
| `[0, 10)`, `[0, 10)` | `1, 1`: identical rows contain one another |
| `[0, 5)`, `[5, 5)` | `1, 0`: the empty row at the right endpoint is contained |
| `[3, 3)`, `[3, 3)` | `1, 1` |
| `[2, 2)`, `[3, 3)` | `0, 0` |

[API reference](api.md#polars_intervals.containment_count) |
[Benchmarks](containment-benchmarks.md)

### Nesting depth

`nesting_depth` measures the longest chain of strict containers **above** each
row. Outermost intervals have depth zero, and each containment step adds one:

```python
intervals = pl.DataFrame({"start": [0, 2, 2, 3], "end": [10, 8, 8, 7]})
result = intervals.lazy().with_columns(pi.nesting_depth("start", "end").alias("depth")).collect()

print(result["depth"].to_list())  # [0, 1, 1, 2]
```

Strict containment uses the same endpoint comparisons as containment counting,
but at least one must be strict. Identical intervals get the same depth and
never add a level. Equal starts with different ends, or equal ends with
different starts, can still form a chain.

Depth measures a chain, not the number of containers. For example, `[0, 8)`
and `[2, 10)` both contain `[4, 5)`, but neither contains the other. The small
interval has two containers and depth **1**.

Empty intervals also follow the endpoint comparisons: `[0, 5)` strictly
contains `[5, 5)`, giving depths `0, 1`.

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

The first and last appointments touch, so they share a lane. Lane IDs start
at zero. Their particular numbering may change with input order or across
releases. Empty intervals receive lane `0`.

[API reference](api.md#polars_intervals.assign_lanes) |
[Benchmarks](assign-lanes-benchmarks.md)

### Balance lane row counts

`assign_balanced_lanes` also uses the fewest lanes, then tries to distribute
the **number of rows** more evenly between them:

```python
intervals = pl.DataFrame({"start": [0, 0, 1, 2, 3, 4], "end": [10, 10, 1, 2, 3, 4]})
result = (
    intervals.lazy().with_columns(pi.assign_balanced_lanes("start", "end").alias("lane")).collect()
)

print(sorted(result["lane"].value_counts()["count"].to_list()))  # [3, 3]
```

The two nonempty intervals require two lanes. Unlike `assign_lanes`, which
puts empty rows in lane `0`, this function may spread them across lanes.

Balancing first minimizes the difference between the largest and smallest
lane row counts, then the sum of squared row counts. It is a heuristic: the
best possible balance is not guaranteed, but the result never scores worse
than `assign_lanes` or a supplied assignment.

To improve an existing assignment, pass its column as `initial_lanes`:

```python
original = intervals.lazy().with_columns(pi.assign_lanes("start", "end").alias("lane"))
rebalanced = original.with_columns(
    pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=100_000).alias("lane")
).collect()
```

Supplied lanes must already be a valid minimum-lane assignment: non-null
integer IDs with a contiguous palette `0..k-1` and no overlaps within a lane.
An invalid assignment raises an error. With groups, it must be valid and
minimum within each group.

`max_work` (default `100_000`) limits refinement work, not elapsed time.
`max_work=0` skips refinement. You can pass a result back as `initial_lanes`
to refine it further.

[API reference](api.md#polars_intervals.assign_balanced_lanes) |
[Measured quality, runtime, and work limits](balance-lanes-benchmarks.md)

## Select intervals by weight

Use these operations when you can choose a subset of the input rows. A weight
represents the value of selecting a row, such as revenue or priority. The
result maximizes the sum of selected weights under the chosen constraint.

Rows with zero or negative weight are never selected, and selecting nothing
is allowed.
All input rows are still validated, including those with nonpositive weights.

The first three algorithms select schedules, progressing from one available
slot to capacity that changes over time. Because empty intervals use no
capacity, they are always selected when their weight is positive. The final
algorithm selects a set of mutually overlapping intervals.

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

`max_weight_with_capacity` allows up to `capacity` selected nonempty intervals
to overlap. Using the same jobs:

```python
chosen = (
    jobs.lazy()
    .filter(pi.max_weight_with_capacity("start", "end", weight="revenue", capacity=2))
    .collect()
)

print(chosen["revenue"].sum())  # 45
```

With two slots, the long job can run alongside all three short jobs. Capacity
one gives the same optimum value as `max_weight_non_overlapping`. `capacity`
must be a nonnegative integer.

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

The integer endpoints represent hours. Capacity is three from 9 to 12, one
from 12 to 14, and four from 14 to 18. The jobs worth 100 and 130 both cross
the noon bottleneck, so only one can be selected. The chosen jobs earn 280.

The returned Series is named `selected` and aligned with the job rows. The two
tables can have different row counts, but all four endpoint columns must share
one dtype.

Profile segments use the same half-open convention as jobs:

- Segments may be unsorted and may touch, but nonempty segments cannot overlap,
  even if their capacities agree.
- Gaps and times outside the profile have zero capacity. A nonempty job
  crossing such a region cannot be selected.
- Capacities must be nonnegative. Zero-length profile segments have no effect.

Use `start`, `end`, `weight`, `profile_start`, `profile_end`, and `capacity`
to choose other column names. If the profile columns already live in `jobs`,
omit the second DataFrame and name those columns:

```python
combined = pl.DataFrame(
    {
        "start": [9, 10, 11],
        "end": [13, 12, 15],
        "weight": [100, 60, 130],
        "cap_start": [9, 12, 14],
        "cap_end": [12, 14, 18],
        "capacity": [3, 1, 4],
    }
)
selected = pi.max_weight_with_capacity_profile(
    combined, profile_start="cap_start", profile_end="cap_end"
)

print(selected.to_list())  # [False, True, True]
```

Job and profile rows still describe independent interval sets. They are not
paired by row. A profile constant at `k` across the job horizon gives the
same optimum value as `max_weight_with_capacity(..., capacity=k)`.

[API reference](api.md#polars_intervals.max_weight_with_capacity_profile) |
[Benchmarks and algorithm notes](capacity-profile-benchmarks.md)

### Select a maximum-weight clique

Use `max_weight_clique` to choose intervals that **all overlap one another**.
For nonempty intervals, the selected rows share a common point. With no
`weight` argument, it selects as many mutually overlapping rows as possible:

```python
intervals = pl.DataFrame({"start": [0, 1, 2, 10], "end": [5, 4, 3, 11], "value": [1, 1, 1, 9]})
most_rows = intervals.lazy().filter(pi.max_weight_clique("start", "end")).collect()
most_value = intervals.lazy().filter(pi.max_weight_clique("start", "end", weight="value")).collect()

print(most_rows["start"].to_list())  # [0, 1, 2]
print(most_value["start"].to_list())  # [10]
```

The first three rows form the largest clique, but the last row alone has
greater weight. The default `weight=None` never reads a column named `weight`.
Explicit weights maximize only total weight, with no secondary preference for
more rows.

Duplicate nonempty rows are distinct members and their weights add. An empty
interval can only be selected alone: it overlaps nothing, not even another
empty interval at the same coordinate.

[API reference](api.md#polars_intervals.max_weight_clique) |
[Benchmarks and correctness notes](clique-benchmarks.md)

## Covering and coverage

Choose a covering operation when the coordinates covered matter. You can
require a continuous target to be covered, maximize total covered length
with a row budget, or choose points that hit every input interval.

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

The target is half-open, just like the input intervals. Touching intervals
can form a continuous cover, and selected intervals may extend beyond the
target. Empty input intervals are never selected. An empty target selects
nothing. A target that cannot be fully covered, in any group, raises an error.

[API reference](api.md#polars_intervals.minimum_cover) |
[Benchmarks](covering-benchmarks.md)

### Cover at minimum cost

`minimum_cost_cover` covers the same kind of target while minimizing total
cost. If costs tie, it chooses fewer intervals:

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
full-length interval. Costs must be zero or positive. Target,
empty-interval, and grouping rules are the same as for `minimum_cover`.

[API reference](api.md#polars_intervals.minimum_cost_cover) |
[Benchmarks](cost-covering-benchmarks.md)

#### Date and Datetime targets

For either covering function, Python integers must fit the integer endpoint
dtype. Python dates work with `Date` endpoints. Python datetimes require
microsecond `Datetime` columns and matching timezone metadata.

To specify another time unit, pass a one-element Series with exactly the
same dtype as the endpoints:

```python
target_start = pl.Series([0], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
target_end = pl.Series([100], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
```

See the [target requirements](api.md#polars_intervals.minimum_cover) for
supported timezones and scalar types.

### Select maximum coverage with a budget

`max_k_coverage` selects at most `k` intervals whose union has the greatest
total length. Overlapping regions count only once, and gaps are allowed:

```python
intervals = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
chosen = intervals.lazy().filter(pi.max_k_coverage("start", "end", k=2)).collect()

print(chosen.rows())  # [(-5, 4), (6, 15)]
```

The selected intervals cover 18 units. Choosing the longest interval `[0, 10)`
first would leave either two-interval combination covering only 15 units.
The function finds an exact optimum.

`k` must be a nonnegative integer. Among equally good solutions, the function
selects the fewest rows, so a large budget still omits redundant ones. Empty
intervals are never selected.

Coverage uses exact physical distance: integer units, days for `Date`, and
the column's `ms`/`us`/`ns` ticks for `Datetime`, including across timezone
transitions.

[API reference](api.md#polars_intervals.max_k_coverage) |
[Benchmarks and algorithm notes](coverage-benchmarks.md)

### Minimum stabbing points

`minimum_stabbing_points` chooses the fewest points needed to hit every
interval. A point hits an interval when `start <= point < end`:

```python
intervals = pl.DataFrame({"start": [0, 2, 5], "end": [4, 6, 9]})
result = (
    intervals.lazy().select(pi.minimum_stabbing_points("start", "end").alias("points")).collect()
)

print(result["points"].to_list())  # [[3, 8]]
```

Point 3 hits the first two intervals, and point 8 hits the third. The output
is **one sorted list**, with the endpoint dtype. For temporal endpoints, the
points use whole days (`Date`) or the column's time ticks (`Datetime`). Empty
input returns one empty list. An empty interval raises an error because it
contains no point.

Use grouped aggregation for one list per group:

```python
grouped = (
    intervals.lazy()
    .with_columns(pl.Series("group", ["a", "a", "b"]))
    .group_by("group", maintain_order=True)
    .agg(pi.minimum_stabbing_points("start", "end").alias("points"))
    .collect()
)

print(grouped.rows())  # [('a', [3]), ('b', [8])]
```

[API reference](api.md#polars_intervals.minimum_stabbing_points) |
[Benchmarks](stabbing-benchmarks.md)

## Choose representative intervals

Use representatives when every input interval needs to be selected or overlap
a selected interval. The selected intervals may overlap or be disjoint.
Unlike covering, there is no continuous target to fill.

### Select a minimum-cost dominating set

`minimum_cost_dominating_set` chooses representatives at minimum total cost.
With no `cost` argument, every row costs one, so it selects the fewest rows:

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

The middle interval overlaps both neighbors and represents itself, so one
row suffices. With explicit prices, selecting both outer intervals costs 2,
compared with 10 for the middle row.

The default `cost=None` never reads a column named `cost`. Explicit costs must
be zero or positive. Among equal-cost solutions, the function
selects the fewest rows. Every empty interval is selected, since nothing else
can represent it.

[API reference](api.md#polars_intervals.minimum_cost_dominating_set) |
[Benchmarks and correctness notes](domination-benchmarks.md)
