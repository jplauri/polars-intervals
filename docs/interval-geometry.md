# Clustering, union, and bounded gaps

Use these operations to group connected intervals, merge covered ranges, or
find gaps in a bounded domain:

| Question | Function | Output |
| --- | --- | --- |
| Which records are connected by overlap? | [`cluster_intervals`](api.md#polars_intervals.cluster_intervals) | One UInt32 component ID per original row |
| Which coordinates are covered? | [`merge_intervals`](api.md#polars_intervals.merge_intervals) | Maximal covered ranges |
| Which coordinates in a domain are uncovered? | [`interval_gaps`](api.md#polars_intervals.interval_gaps) | Maximal uncovered ranges |

All three follow the shared [input rules](usage.md#inputs).

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
