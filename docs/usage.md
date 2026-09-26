# Usage

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
