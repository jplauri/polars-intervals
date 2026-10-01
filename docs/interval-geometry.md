# Interval geometry

Use these operations to group connected intervals, merge ranges, find gaps,
subtract exclusions, or intersect two collections:

| Question | Function | Output |
| --- | --- | --- |
| Which records are connected by overlap? | [`cluster_intervals`](api.md#polars_intervals.cluster_intervals) | One UInt32 component ID per original row |
| Which coordinates are covered? | [`merge_intervals`](api.md#polars_intervals.merge_intervals) | Maximal covered ranges |
| Which coordinates in a domain are uncovered? | [`interval_gaps`](api.md#polars_intervals.interval_gaps) | Maximal uncovered ranges |
| Which left coordinates remain after exclusions? | [`subtract_intervals`](api.md#polars_intervals.subtract_intervals) | Maximal remaining ranges |
| Which coordinates do both collections cover? | [`intersect_intervals`](api.md#polars_intervals.intersect_intervals) | Maximal shared ranges |

All operations follow the shared [input rules](usage.md#inputs).

## A worked example

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [1, 3, 5, 8, 12], "end": [4, 6, 8, 10, 15]})
result = df.with_columns(
    pi.cluster_intervals("start", "end").alias("strict"),
    pi.cluster_intervals("start", "end", include_touching=True).alias("touching"),
)
print(result["strict"].to_list())  # [0, 0, 0, 1, 2]
print(result["touching"].to_list())  # [0, 0, 0, 0, 1]
print(pi.merge_intervals(df).rows())
# [(1, 10), (12, 15)]
print(pi.interval_gaps(df, domain_start=0, domain_end=16).rows())
# [(0, 1), (10, 12), (15, 16)]
```

The first three rows are connected through overlaps. They need not all share
one point. The fourth row only touches that component at 8, so it joins only
with `include_touching=True`. Union always joins touching ranges because there
is no uncovered coordinate between them.

Cluster IDs follow each component's first appearance in the original input.
For `[10,12), [0,2), [1,3)`, they are `[0,1,1]`. Permuting input rows may
renumber components while preserving their membership. IDs restart at zero
inside each group.

## Empty rows and domains

Every `[x,x)` row is an isolated cluster, including duplicate empty rows and
touching-inclusive clustering. It never connects two nonempty rows. Empty rows
contribute nothing to union and never split a gap.

| Input | Clusters | Union | Gaps in a nonempty domain D |
| --- | --- | --- | --- |
| No rows, ungrouped | Empty UInt32 column | No rows | D |
| Three empty rows, ungrouped | `[0,1,2]` | No rows | D |
| No rows, grouped | No observed groups | No rows | No rows |

Both gap bounds are required. The same scalar domain applies to every observed
group. Bounds follow the [scalar rules of `minimum_cover`](api.md#polars_intervals.minimum_cover).
Equal bounds return no gaps. Reversed bounds are invalid.

All original rows are validated before clipping, including rows outside the
domain and calls with an empty domain. A group with no coverage inside the
domain returns the whole domain. Only groups present in the input produce
gaps. Gaps include uncovered leading and trailing regions within the bounds.

## Grouping and schemas

```python
grouped = df.with_columns(pl.Series("resource", ["a", "a", "a", "b", "b"]))
labels = grouped.with_columns(
    pi.cluster_intervals("start", "end").over("resource").alias("cluster")
)
lists = grouped.group_by("resource", maintain_order=True).agg(
    pi.cluster_intervals("start", "end").alias("clusters")
)
merged = pi.merge_intervals(grouped, by="resource")
gaps = pi.interval_gaps(grouped, by="resource", domain_start=0, domain_end=16)
```

Union and gaps return `[requested keys..., "start", "end"]`, even with custom
input endpoint names. Endpoints keep their exact dtypes, including when no
segments remain. Segments inside each group are sorted by start, nonempty, and
strictly separated. Other input columns are omitted.

See the shared [frame and grouping rules](usage.md#frame-results). Union and
gaps reserve the output names `start` and `end`, so group keys cannot use them.

## Eager and lazy execution

This pipeline computes gaps for each resource, then keeps gaps starting at 8
or later:

```python
query = (
    grouped.lazy()
    .pipe(pi.interval_gaps, by="resource", domain_start=0, domain_end=16)
    .filter(pl.col("start") >= 8)
    .select("resource", "start", "end")
)
result = query.collect(engine="streaming")
```

Filtering before an operation changes the intervals it sees. Filtering after
it selects computed results. See [frame execution](usage.md#frame-results)
for lazy and streaming behavior.

The [API reference](api.md) has full signatures and edge cases. The
[benchmark report](interval-geometry-benchmarks.md) covers runtime, memory use,
and algorithm details.

## Subtract and intersect two collections

Subtract busy windows from availability, or find times when two resources are
both available. Both functions act on the union represented by each collection:

```python
left = pl.DataFrame({"start": [0, 4, 12], "end": [5, 10, 15]})
right = pl.DataFrame({"start": [2, 6, 10], "end": [3, 8, 13]})

print(pi.subtract_intervals(left, right).rows())
# [(0, 2), (3, 6), (8, 10), (13, 15)]
print(pi.intersect_intervals(left, right).rows())
# [(2, 3), (6, 8), (12, 13)]
```

The original left boundary at 5 disappears. Each output contains maximal,
nonempty, disjoint ranges. Overlapping and touching fragments coalesce, and
each end is strictly below the next start. Touching inputs alone have no
intersection: `[0,2)` and `[2,4)` share no covered coordinates.

Subtraction cuts away coverage, so one left row may produce several ranges.
It is not a whole-row anti-join or subtraction that preserves each source
record's fragments. Intersection does not enumerate overlapping record pairs.
Neither operation returns source IDs, payload columns, or metadata aggregation.
These are geometric set operations, not a BEDTools compatibility API.

### Availability by resource

```python
availability = pl.DataFrame(
    {"resource": ["desk", "room", "desk"], "start": [0, 0, 8], "end": [6, 10, 12]}
)
busy = pl.DataFrame({"resource": ["desk"], "start": [2], "end": [4]})
print(pi.subtract_intervals(availability, busy, by="resource").rows())
# [('desk', 0, 2), ('desk', 4, 6), ('desk', 8, 12), ('room', 0, 10)]
```

The room has no busy rows, so subtraction keeps its union. A left-only group
has no intersection. A right-only group emits nothing. Null key values match
nulls, including in multi-column keys. There is no global right-side broadcast.
Both sides use the same key names. Rename keys upstream if needed.

Groups follow their first appearance in the original left input, before empty
rows are dropped. Ranges are sorted within each group. Swapping intersection's
operands preserves geometry per key but can change group-block order.

Empty rows contribute nothing, and duplicates do not change coverage. Empty
left input returns a typed empty result. Empty right input leaves the left
union for subtraction and returns no intersection. Every row on both sides
still validates, including right-only groups and calls with an empty operand.
Errors identify the side and original row within the evaluated operand.

### Lazy and mixed inputs

```python
query = availability.lazy().pipe(pi.subtract_intervals, busy.lazy(), by="resource")
result = query.filter(pl.col("start") >= 4).collect(engine="streaming")

# An eager operand becomes an in-memory source in the same deferred plan.
shared = pi.intersect_intervals(availability, busy.lazy(), by="resource")
result = shared.collect()
```

Two DataFrames return a DataFrame. If either argument is a LazyFrame, the
result stays lazy. Construction, `explain()` and `collect_schema()` read no
input rows. Both sources enter one blocking native operation. Collection with
the streaming engine also materializes both inputs. This is not bounded-memory
streaming. Downstream filters, slices and projections apply to the completed
geometry. Upstream operations change the intervals that the function sees.

Use `left_start`, `left_end`, `right_start` and `right_end` for custom literal
column names. Output names remain `start` and `end`. All four endpoints must
have the same supported dtype, including Datetime units and timezone metadata.
Corresponding key dtypes must match. These checks also apply to typed empty
frames. No casts or inferred Null endpoints are accepted. Exact logical types
survive empty results. Keys cannot be named `start` or `end`.

See the [subtraction and intersection benchmarks](set-geometry-benchmarks.md)
for complete eager/lazy timings and the production algorithm comparison.
