# Usage

Most interval operations take column names or Polars expressions for eager or
lazy queries. Capacity-profile selection takes two eager DataFrames and returns
a Series. See the [API reference](api.md) for full parameter and return types.

## Select maximum coverage with a budget

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
selected = df.filter(pi.max_k_coverage("start", "end", k=2))
# [-5,4) and [6,15): 18 units of coverage.
```

Select at most `k` intervals whose union has maximum total measure. Among
maximum-coverage solutions, use the fewest intervals. The Boolean expression
also works in `select`, `with_columns`, lazy queries, and `.over("group")`;
each group independently gets the same scalar budget.

Intervals are half-open `[start,end)`. Empty intervals never contribute and are
never selected. Empty input returns an empty Boolean mask; `k=0` returns all
False after validation. `k>=n` attains the complete union but still omits
redundant rows. Results are deterministic for identical input and preserve row
order; no particular optimum mask is guaranteed when both objectives tie.

Endpoints must have exactly matching integer, Date or Datetime dtypes,
including Datetime unit and timezone. Nulls and reversed intervals are rejected;
no implicit coercion occurs. Exact physical distance uses integer units, days
for Date and ms/us/ns ticks for Datetime, even across timezone transitions.

Two tempting greedy rules fail:

- Longest-first: `A=[0,10)`, `B=[-5,4)`, `C=[6,15)`, `k=2`.
  `A+B` or `A+C` covers 15, but `B+C` covers 18.
- Top-k by length: `A=[0,10)`, `B=[1,11)`, `C=[10,18)`, `k=2`.
  The two length-10 intervals cover only 11; `A+C` covers 18.

The implementation uses the exact Li et al. offline dynamic program, not a
greedy approximation. Worst-case time is `O(n log n + min(k,n)n)` and space
is `O(n + min(k,n)n)`, with rolling objective rows and compact decisions.
Validated zero/one budgets take `O(n)`; a sufficient budget uses a minimum
full-union cover after sorting. See the [benchmarks and linked design notes](coverage-benchmarks.md)
for measured variants, memory scaling, proofs, and literature citations.

## Count containment

Count how many other intervals are contained by each row:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 2, 4], "end": [10, 5, 12]})
result = df.with_columns(pi.containment_count("start", "end").alias("contained"))
# contained: [1, 0, 0]
```

Containment is defined by **both** `A.start <= B.start` and `B.end <= A.end`,
with self excluded. Intervals retain the half-open representation `[start, end)`.
The endpoint predicate applies even to empty intervals.

```text
A [0,10)
B   [2,5)
C       [4,12)

overlap_count(A)      = 2
containment_count(A)  = 1
```

| Case | Containment behavior |
| --- | --- |
| m identical rows | Each counts the other m - 1 rows |
| `[0, 10)` and `[5, 5)` | The outer row contains the empty row |
| `[0, 5)` and `[5, 5)` | The outer row contains the empty row at its right endpoint |
| Two `[3, 3)` rows | Each contains the other |
| `[2, 2)` and `[3, 3)` | Neither contains the other |
| `[0, 5)` and `[5, 10)` | Neither contains the other |
| `[0, 5)` and `[3, 8)` | They overlap, but neither contains the other |

Containment endpoints may be signed or unsigned 8/16/32/64-bit integers, Date,
or Datetime with ms/us/ns units and optional timezones. Logical dtypes must match
exactly, including datetime unit and timezone. Nulls, unequal lengths and
`start > end` are rejected without coercion or scalar broadcasting.

The result is a non-null `UInt64` expression aligned to the original rows across
all chunks. Empty input returns empty output. Both eager and lazy queries work.
For group-local counts, use:

```python
result = (
    df.lazy()
    .with_columns(pi.containment_count("start", "end").over("group").alias("contained"))
    .collect()
)
```

Here `df` must also contain a `group` column. `group_by("group").agg(...)`
returns one list of counts per group. Filtering before counting changes which
intervals participate; filtering after counting preserves the computed counts.

The exact algorithm is an offline 2D dominance sweep: packed records sorted by
descending start, compressed ends, and inclusive Fenwick prefix sums. All rows
with the same start are inserted before their queries. It takes `O(n log n)`
time and `O(n)` additional space without enumerating containment pairs.
See [benchmark details](containment-benchmarks.md) for measurements and native
Polars alternatives.

## Nesting depth

Return the length of the longest strict containment chain above each interval.
**Outermost intervals have depth 0.** Each strict containment step adds one.

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 2, 4, 6], "end": [20, 15, 10, 8]})
result = df.with_columns(pi.nesting_depth("start", "end").alias("depth"))
# depth: [0, 1, 2, 3]
```

`A` strictly contains `B` exactly when:

```text
A.start <= B.start and B.end <= A.end
and (A.start < B.start or B.end < A.end)
```

Equal starts or equal ends can therefore form a chain. Identical geometries
cannot: every duplicate receives the same depth and never creates an extra level.

| Input intervals, in row order | Depths |
| --- | --- |
| `[1,10)`, `[1,8)`, `[1,5)` | `0, 1, 2` |
| `[0,10)`, `[2,10)`, `[5,10)` | `0, 1, 2` |
| `[0,10)`, `[2,8)`, `[2,8)`, `[3,7)` | `0, 1, 1, 2` |
| `[0,10)`, `[0,10)`, `[2,8)` | `0, 0, 1` |
| `[0,5)`, `[5,5)` | `0, 1` |
| `[3,3)`, `[3,3)` | `0, 0` |

Empty intervals use the **endpoint predicate**, including at an outer interval's
right endpoint. Thus `[0,5)` strictly contains `[5,5)` even though intervals use
half-open notation. Identical empty intervals do not contain one another strictly.
Crossings such as `[0,5)`, `[3,8)`, `[6,10)` all have depth zero.

Nesting depth differs from `containment_count`, which counts rows **inside** each
interval and includes duplicate geometries. It also differs from the number of
strict containers **above** a row: `[0,8)`, `[2,10)`, `[3,11)` all contain `[4,5)`,
but are mutually incomparable. The small row has three strict containers and
depth **1**. Depth measures a longest chain, not the number of containing rows.

The non-null `UInt64` output is deterministic and aligned to original row order,
including across multiple chunks. Empty input returns empty output. Eager
`select`/`with_columns` and lazy queries work. Each window is independent:

```python
grouped = pl.DataFrame({"group": ["a", "b", "a", "b"], "start": [0, 2, 2, 4], "end": [10, 8, 8, 6]})
result = (
    grouped.lazy()
    .with_columns(pi.nesting_depth("start", "end").over("group").alias("depth"))
    .collect()
)
# depth: [0, 0, 1, 1]
```

`group_by("group").agg(pi.nesting_depth("start", "end"))` returns one depth list
per group, in that group's input order. Filtering before evaluation changes the
participating intervals; filtering afterward preserves the calculated depths.

Endpoint dtypes must match exactly: signed/unsigned 8/16/32/64-bit integers,
Date, or Datetime ms/us/ns, including matching timezone metadata. Nulls, unequal
lengths and `start > end` are rejected with no implicit coercion or broadcasting.
Reversed intervals report the first original zero-based row within the collection.
Temporal values use the existing physical integer representation without endpoint
arithmetic. The Polars-independent Rust API is
`intervals_core::nesting_depths(&starts, &ends)`; Rust Polars exposes
`polars_intervals::nesting_depth(&starts, &ends)`.

The production kernel sorts packed records by start ascending, end descending,
then original index. A monotone vector records the greatest achievable ending
endpoint at each chain length. It uses binary search to find a predecessor,
or appends directly when the deepest chain can extend. Exact duplicate geometries
share one query and update. Total time is `O(n log n)` and additional space is
`O(n)`, with only `O(max_depth + 1)` frontier entries. No containment pairs or
coordinate compression are needed. The production code uses the standard sort
without an additional sorted-input scan.
See the
[benchmarks and linked design notes](nesting-depth-benchmarks.md)
for memory tradeoffs and the sorted-input investigation.

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

## Balance lane row counts

`assign_balanced_lanes` assigns the minimum number of nonoverlapping lanes and
heuristically balances the **number of rows** in them. Its keyword-only
`initial_lanes=None` default constructs an assignment; supply a lane column or
expression to improve an existing proper minimum-lane assignment:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 0, 1, 2, 3, 4], "end": [10, 10, 1, 2, 3, 4]})
result = df.with_columns(pi.assign_balanced_lanes("start", "end").alias("lane"))
# Two lanes, three rows each: the four empty rows can use either lane.

original = df.with_columns(pi.assign_lanes("start", "end").alias("lane"))
repaired = original.with_columns(
    pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=100_000).alias("lane")
)
```

For lane sizes `s`, the objective is lexicographic: first minimize
`D = max(s) - min(s)`, then `Q = sum(size * size for size in s)`. Decisions use
exact integers. A smaller spread wins even if its squared-size sum is larger.
Occupied duration and endpoint span do not enter the objective. Properness and
minimum lane count are guaranteed; globally optimal balance and approximation
ratios are **not**. With `initial_lanes=None`, the result never worsens `(D, Q)`
relative to `assign_lanes`. With supplied `initial_lanes`, it never worsens the
supplied coloring's score and preserves that coloring's palette.

**Empty-row difference:** `assign_lanes` always puts empty rows in lane zero.
`assign_balanced_lanes` counts every row and may put empties in any existing lane,
even when the empty point lies inside another interval. Empties conflict
with nothing. Touching nonempty intervals do not overlap either. No extra lanes
are introduced: empty input uses zero lanes; other inputs use
`max(1, maximum_nonempty_concurrency)`, including one lane for an empty-only input.

The balancing expression accepts column names or expressions and returns non-null
`UInt32` IDs in original row order. Endpoints must have exactly matching Int8/16/32/64,
UInt8/16/32/64, Date, or Datetime dtypes, including time unit and timezone.
Nulls, unequal lengths and reversed intervals are errors; reversed intervals
report the first original row index. There is no coercion or scalar broadcasting.
Supplied lanes must be non-null 8/16/32/64-bit integers in `0..2**32-1`, with a
contiguous palette `0..k-1`, no within-lane conflicts, and the true minimum `k`.
Boolean, floating-point and temporal lane columns, sparse IDs and proper
assignments with extra lanes are rejected. Invalid supplied assignments are
never replaced with a coloring constructed from scratch.

The finite default `max_work=100_000` bounds deterministic refinement work, not
elapsed time. It accepts integers from zero through `2**64-1`, excluding Boolean.
Validation and fixed `O(n log n)` preprocessing/construction are outside this
budget. Refinement charges for candidate-pair scans, row visits, bitset words
and reconstruction; it can stop before attempting a costly exact pair. Scratch
space is linear in the pair size, preflighted against the remaining budget,
and allocated with checked sizes. See the
[algorithm and work-accounting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/balance-lanes-notes.md)
for units, limits and complexity. At zero budget, `initial_lanes=None` returns the
existing baseline; supplied `initial_lanes` returns those IDs unchanged, **after
all validation**, including empty and already-balanced inputs. Another call with
the result as `initial_lanes` can improve a budget-limited result further.

Lazy and eager queries, slices and streaming collection have the same whole-input
semantics across all chunks. Use `.over("group")` to solve each complete group
independently with its own budget, or `group_by("group").agg(...)` for one lane
list per group. Supplied lanes must be minimum and proper within each such group.
Identical input/options give identical output, including equivalent chunk layouts
and order-preserving endpoint representations. Row permutations can change labels
and heuristic quality because original row indices break ties.

[API reference](api.md#polars_intervals.assign_balanced_lanes) ·
[Measured quality and runtime](balance-lanes-benchmarks.md)

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

## Select with a capacity profile

Select a globally maximum-weight subset of fixed intervals subject to a
piecewise-constant capacity profile:

```python
import polars as pl
import polars_intervals as pi

# Integer hours keep this example compact; matching Date/Datetime columns work too.
jobs = pl.DataFrame(
    {"start": [9, 10, 11, 14], "end": [13, 12, 15, 17], "weight": [100, 60, 130, 90]}
)
profile = pl.DataFrame({"start": [9, 12, 14], "end": [12, 14, 18], "capacity": [3, 1, 4]})
selected = pi.max_weight_with_capacity_profile(jobs, profile)
print(selected.to_list())  # [False, True, True, True]
result = jobs.filter(selected)
```

The capacity is three from 09–12, one from 12–14, and four from 14–18.
The jobs worth 100 and 130 both cross the noon bottleneck, so only one can
survive it. The selected subset has total weight 280. Optimization is global
and exact; sorting jobs by weight and greedily accepting them is not equivalent.

This eager package function accepts one or two DataFrames. Separate frames can
have independent row counts. It returns a non-null Boolean Series named
`selected`, aligned with the original job rows. Use `start`, `end`, `weight`,
`profile_start`, `profile_end`, and `capacity` to choose other column names.
For lazy inputs, explicitly collect each input first; this function does not
hide materialization inside an expression or perform optimization in Python.

Omitting `capacity_profile`, or passing `None`, reads the profile columns from
`jobs`:

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
    combined,
    profile_start="cap_start",
    profile_end="cap_end",
)
print(selected.to_list())  # [False, True, True]
```

The job and profile columns still describe independent interval sets. Rows are
not paired: each job uses the capacity function across its entire lifetime,
regardless of which profile segment shares its row index.

- Jobs and profile rows are half-open `[start, end)`. Touching segments are valid.
- Profile rows may be unsorted. Overlapping non-empty rows are rejected as
  ambiguous, even if their capacities agree or are zero.
- Gaps and times outside the supplied profile have zero capacity. A non-empty
  job crossing any such region cannot be selected.
- Valid zero-length profile rows have no effect. Adjacent segments with the
  same capacity may be coalesced internally.
- All four endpoint columns must have exactly the same supported logical dtype:
  signed/unsigned 8/16/32/64-bit integer, Date, or Datetime. Datetime units
  (`ms`, `us`, `ns`) and timezone metadata must match exactly.
- Weights and capacities are non-null signed/unsigned integers up to 64 bits.
  Negative capacities, floats, Decimal, Int128 columns, null endpoints, and
  reversed intervals are rejected. There are no implicit casts.
- Objective arithmetic uses checked `i128`. Positive empty jobs are always
  selected and consume no capacity, including outside the profile. Nonpositive
  jobs are omitted. Identical inputs produce identical masks; choices between
  tied optimal subsets are not guaranteed across releases.
- Capacity above the number of positive non-empty candidate jobs is clamped
  internally: no selection can exceed that concurrency, so the optimum is unchanged.

A profile constant at `k` across the relevant job horizon has the same optimum
as `max_weight_with_capacity(..., capacity=k)` and dispatches to that existing
kernel. Capacity one consequently reaches `max_weight_non_overlapping`.
Empty/zero profiles and instances where all positive jobs fit avoid flow.
The compact timeline contains endpoints, never every elapsed Date/Datetime tick.
See the [benchmarks and linked design notes](capacity-profile-benchmarks.md)
for measurements, proofs, and the production solver's component fast paths.

The Rust Polars API takes six `&Series` arguments in job start/end/weight,
profile start/end/capacity order. The Polars-independent core API takes six
slices in that same order; endpoint types need ordering, copying, and `Sync`.

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
