# Usage

## Cover one continuous target

`minimum_cover` selects the fewest intervals whose union continuously covers a
target. It is an exact global optimization. The furthest-reaching greedy choice
selects `[0,6)` followed by `[6,10)` here:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 0, 4, 6, 7], "end": [4, 6, 7, 10, 10]})
df.filter(pi.minimum_cover("start", "end", target_start=0, target_end=10))
```

Choosing `[0,4)` first can require three intervals. The greedy rule compares
every eligible interval and takes the one reaching furthest right.

`minimum_cost_cover` instead minimizes total cost, then uses fewer intervals
among covers with the same cost:

```python
df = pl.DataFrame({"start": [0, 0, 5], "end": [10, 5, 10], "cost": [100, 10, 10]})
df.filter(
    pi.minimum_cost_cover(
        "start",
        "end",
        cost="cost",
        target_start=0,
        target_end=10,
    )
)
# Selects [0,5) and [5,10): cost 20 instead of the single interval costing 100.
```

Both targets and intervals are half-open. Touching intervals chain perfectly;
empty intervals never help. Intervals extending outside the target are allowed.
An empty target selects nothing. Reversed targets/intervals and null endpoints
are errors, even for empty targets. An infeasible non-empty target raises
`target interval cannot be covered by the supplied intervals`.

Costs accept nonnegative Int8/16/32/64 and UInt8/16/32/64, without nulls, floats,
Decimal or implicit casts. Totals use exact checked `i128` arithmetic. Core
inputs can exercise the full accumulator range; overflow is reported if the
minimum feasible cost cannot be represented. Ties are deterministic for
identical input, but a particular tied mask is not a stable API promise.

Targets are scalar configuration. Python integers must fit the endpoint dtype.
Python dates require Date columns; Python datetimes require Datetime with
microsecond units and identical timezone metadata. For explicit units and full
nanosecond precision, pass a **one-element typed Series** for each scalar:

```python
target_start = pl.Series([0], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
target_end = pl.Series([100], dtype=pl.Int64).cast(pl.Datetime("ns", "UTC"))
```

Typed targets must match exactly, including integer width, Datetime unit and
timezone. Date/Datetime, different units, incompatible timezones and lossy
numeric conversions are rejected. Target values are not broadcast into columns.
Python datetimes accept naive, UTC, or named timezone metadata; fixed-offset or
custom timezone objects require an explicitly typed Series to avoid implicit
timezone normalization by the scalar constructor.

Use the expressions in eager `select`, lazy `with_columns`, or direct `filter`.
`.over("group")` solves each group independently against the same target scalars;
one infeasible group raises an error. All chunks form one instance. Filtering
before selection changes the available intervals.

Both algorithms take `O(n log n)` time and `O(n)` additional space. The unweighted
solver sorts packed candidates and sweeps linearly; the weighted solver uses
frontier dynamic programming with a reversed Fenwick suffix-min tree and
backpointers. See the [candidate benchmark comparison](covering-benchmarks.md).

`overlap_count`, `assign_lanes`, and `max_weight_non_overlapping` accept column names or Polars expressions.
Use them in `select` or `with_columns` on eager or lazy frames. The examples
below use lazy queries.

## Select a globally maximum-weight schedule

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "revenue": [15, 10, 10, 10],
    }
)
chosen = df.filter(pi.max_weight_non_overlapping("start", "end", weight="revenue"))
assert chosen["revenue"].sum() == 30
```

The intervals `[0, 4)`, `[4, 7)`, and `[7, 10)` together earn 30, beating
`[0, 10)` at 15. Choosing the largest individual weight, or deciding each row
independently, cannot solve this global optimization. Earliest-finish greedy is
also insufficient: `[0, 1)` at weight 1 loses to `[0, 4)` at weight 20.

The output is a non-null Boolean mask in original row order. Selection is exact
and deterministic for identical input; which optimal subset wins a tie is not
part of the stable contract. The empty subset has value zero. Negative and
zero-weight rows are omitted. Positive empty intervals `[x, x)` conflict with
nothing and are all selected, including several at the same coordinate.
Touching non-empty intervals are compatible under `[start, end)` semantics.

Weights accept only `Int8/16/32/64` and `UInt8/16/32/64`, with no nulls or
implicit casts. Objectives use checked `i128` arithmetic, returning an error
instead of overflowing. Float, Decimal, Boolean and temporal weights are
unsupported; floating-point weights may be considered separately. Endpoints
follow the [input rules](#inputs), including Date and Datetime support.

```python
schedule = pi.max_weight_non_overlapping(
    pl.col("start"),
    pl.col("end"),
    weight=pl.col("revenue") * 2,
).alias("selected")
result = df.lazy().with_columns(schedule).collect()
```

Use `schedule.over("group")` for independent optimization within each group.
`df.group_by("group").agg(schedule)` returns `List(Boolean)` masks per group.
All chunks form one instance; the full instance is needed even with the
streaming engine. Filtering before optimization changes the problem being solved.

The production DP takes `O(n log n)` time and `O(n)` additional space.
See the [candidate comparison](weighted-scheduling-benchmarks.md) for the measured
algorithm choice, and the [API reference](api.md) for validation details.

## Assign the minimum number of lanes

`assign_lanes` supports calendar/timeline layout, machine/resource lanes,
Gantt charts, genomic tracks, and concurrent-job visualization.

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [0, 1, 2], "end": [2, 3, 4]})
result = df.lazy().with_columns(pi.assign_lanes("start", "end").alias("lane")).collect()
assert result["lane"].dtype == pl.UInt32
assert result["lane"].n_unique() == 2
assert result["lane"][0] == result["lane"][2]
```

The first and last intervals touch, so they can share a lane. For non-empty
intervals, **minimum number of lanes = maximum concurrency**. Empty intervals
consume no capacity and receive lane `0`. Nonempty input consisting only of
empties uses one lane; empty input returns empty output.

IDs are contiguous `0..k-1`, returned in original row order, and deterministic
for identical input. No particular optimal coloring or stable lane numbers
across releases or input permutations are promised. Filtering before assignment
changes the collection being colored; filtering afterwards keeps its assigned IDs
and can therefore leave gaps in the filtered result.

Use `pi.assign_lanes("start", "end").over("group")` for independent assignments
per group. `group_by("group").agg(pi.assign_lanes("start", "end").alias("lanes"))`
returns `List(UInt32)` per group. Eager `select`, lazy `with_columns`, multiple
chunks, and temporal endpoints all follow the same [input rules](#inputs).
The whole collection is needed even when collecting with the streaming engine.

See the [API reference](api.md) and [algorithm comparison](assign-lanes-benchmarks.md).

## Count within groups

Use `.over(...)` to compare intervals that share a group, such as ranges on the
same chromosome or positions in the same file.

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "group": ["a", "b", "a", "b"],
        "start": [1, 1, 2, 5],
        "end": [4, 4, 3, 6],
    }
)
result = (
    df.lazy()
    .with_columns(pi.overlap_count("start", "end").over("group").alias("overlaps"))
    .collect()
)
print(result)
```

```text
shape: (4, 4)
┌───────┬───────┬─────┬──────────┐
│ group ┆ start ┆ end ┆ overlaps │
│ ---   ┆ ---   ┆ --- ┆ ---      │
│ str   ┆ i64   ┆ i64 ┆ u64      │
╞═══════╪═══════╪═════╪══════════╡
│ a     ┆ 1     ┆ 4   ┆ 1        │
│ b     ┆ 1     ┆ 4   ┆ 0        │
│ a     ┆ 2     ┆ 3   ┆ 1        │
│ b     ┆ 5     ┆ 6   ┆ 0        │
└───────┴───────┴─────┴──────────┘
```

The two intervals in group `a` overlap. The two in group `b` do not.
`.over("group")` keeps the original rows and their order.

To collect the counts into one list per group, use `group_by(...).agg(...)`:

```python
grouped = (
    df.lazy()
    .group_by("group", maintain_order=True)
    .agg(pi.overlap_count("start", "end").alias("overlaps"))
    .collect()
)
print(grouped)
```

```text
shape: (2, 2)
┌───────┬───────────┐
│ group ┆ overlaps  │
│ ---   ┆ ---       │
│ str   ┆ list[u64] │
╞═══════╪═══════════╡
│ a     ┆ [1, 1]    │
│ b     ┆ [0, 0]    │
└───────┴───────────┘
```

## Choose which rows to compare

Filtering before counting limits the intervals used in the comparison.
Filtering afterwards keeps counts computed against the full input.

```python
df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
count = pi.overlap_count("start", "end").alias("overlaps")
keep = pl.col("start") < 3

within_subset = df.lazy().filter(keep).with_columns(count).collect()
against_all = df.lazy().with_columns(count).filter(keep).collect()

print(within_subset["overlaps"].to_list())
print(against_all["overlaps"].to_list())
```

```text
[1, 1, 0]
[1, 2, 0]
```

The interval `[2, 4)` has one overlap in the subset and two in the full input.

## Interval rules

Intervals are half-open: `[start, end)`. Two non-empty intervals overlap when
`start < other_end` and `other_start < end`. Each row excludes itself.

| Case | Behavior |
| --- | --- |
| `[1, 3)` and `[3, 5)` | Touching endpoints do not overlap |
| `[1, 4)` and `[3, 5)` | Each counts the other |
| `[2, 2)` | Empty; counts zero and contributes no overlaps |
| Two rows containing `[1, 4)` | Separate intervals; each counts the other |
| A single interval | Counts zero |

## Inputs

Endpoints must have matching integer, `Date`, or `Datetime` dtypes, contain no
nulls, and satisfy `start <= end`. Both endpoints must be `Date`, or both must
be `Datetime` with the same time unit (`ms`, `us`, or `ns`) and exactly matching
timezone metadata. Matching timezone-aware columns are supported; naive/aware
pairs are rejected, as are Date/Datetime, temporal/integer, and different units
or timezones. No automatic coercion is performed. `Time`, `Duration`, and
floating-point columns are not supported. See the [temporal example](api.md)
in the API reference.

If integer widths differ, cast both inputs explicitly:

```python
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count(
            pl.col("start").cast(pl.Int64),
            pl.col("end").cast(pl.Int64),
        ).alias("overlaps")
    )
    .collect()
)
```

Choose a type that can hold every endpoint; for example, some `UInt64` values
cannot fit in `Int64`. See the [API reference](api.md) for all supported types
and validation errors.

Counts use the entire input, or each group when grouped. Collecting with Polars'
streaming engine still requires the counting operation to see that collection;
it uses memory proportional to its size.

## Select with a simultaneous capacity

`max_weight_with_capacity` selects a globally maximum-weight subset of intervals
subject to a maximum simultaneous capacity. It returns a Boolean expression in
original row order, suitable for `select`, `with_columns`, or `filter`:

```python
import polars as pl
import polars_intervals as pi

jobs = pl.DataFrame(
    {
        "start": [0, 0, 4, 7],
        "end": [10, 4, 7, 10],
        "weight": [15, 10, 10, 10],
    }
)
selected = jobs.filter(pi.max_weight_with_capacity("start", "end", weight="weight", capacity=2))
assert selected["weight"].sum() == 45
```

At capacity 1 the three short intervals give 30, exactly the optimum of
`max_weight_non_overlapping`. At capacity 2 the long interval can coexist with
that schedule, giving 45. The optimization is globally exact.

Intervals are half-open `[start, end)`, so touching intervals do not overlap.
Capacity is a nonnegative integer; zero selects only positive empty intervals.
Positive empty intervals always consume zero capacity. Negative and zero-weight
rows are omitted. The empty subset is allowed with objective zero. Tie choices
are deterministic for identical input, but a particular optimal mask is not a
stable public contract.

Weights must be non-null signed or unsigned integers up to 64 bits. Objectives
use exact, checked `i128` accumulation; there are no implicit casts or floating
weights. Endpoints support matching integer, Date, and Datetime dtypes, including
matching Datetime units/timezones. Nulls, reversed intervals and unequal input
lengths are rejected; reversed intervals report the original row index.

Use `.over("group")` for independent group optimization; grouped aggregation
returns Boolean lists. All chunks form a single instance. Filtering before the
expression changes the optimization problem.

The core preserves the specialized capacity-1 dynamic program. It removes
irrelevant rows, accepts all positive candidates when capacity is sufficient,
and splits independent overlap components before exact successive-shortest-path
min-cost flow. For n rows and constrained component sizes n_c, runtime is
`O(n log n + sum(capacity * n_c * log(n_c + 1)))`, with `O(n)` additional memory.
Capacity zero is linear; capacity one is `O(n log n)`.

Substantial independent component workloads use at most eight Rust workers;
small inputs and single components stay serial. See the [algorithm and benchmark
report](capacity-scheduling-benchmarks.md) for measured tradeoffs and reproduction.
