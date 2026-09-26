# Usage

`overlap_count` and `assign_lanes` accept column names or Polars expressions.
Use them in `select` or `with_columns` on eager or lazy frames. The examples
below use lazy queries.

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
