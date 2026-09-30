# Clustering, union, and bounded gaps

These operations answer three different questions about half-open intervals:

| Question | Function | Output |
| --- | --- | --- |
| Which records are connected by overlap? | [`cluster_intervals`](api.md#polars_intervals.cluster_intervals) | One UInt32 component ID per original row |
| Which coordinates are covered? | [`merge_intervals`](api.md#polars_intervals.merge_intervals) | Maximal covered ranges |
| Which coordinates in a domain are uncovered? | [`interval_gaps`](api.md#polars_intervals.interval_gaps) | Maximal uncovered ranges |

They transform interval geometry. They do not select an optimal subset or
retain arbitrary metadata in the segment outputs.

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
one point. The fourth row only touches that component at 8. Union always joins
touching ranges because there is no uncovered coordinate between them.

Cluster IDs follow each component's first appearance in the original input.
For `[10,12), [0,2), [1,3)`, they are `[0,1,1]`. Permuting input rows may
renumber components while preserving their membership. IDs restart at zero
inside each group. Use `.alias(...)` to name the expression result.

## Empty rows and domains

Every `[x,x)` row is an isolated cluster, including duplicate empty rows and
touching-inclusive clustering. It never connects two nonempty rows. Empty rows
contribute nothing to union and never split a gap.

| Input | Clusters | Union | Gaps in a nonempty domain D |
| --- | --- | --- | --- |
| No rows, ungrouped | Empty UInt32 column | No rows | D |
| Three empty rows, ungrouped | `[0,1,2]` | No rows | D |
| No rows, grouped | No observed groups | No rows | No rows |
| Observed group with only empty or out-of-domain rows | Every empty row is isolated | Union ignores empty rows | D |

Both gap bounds are required. The same scalar domain applies to every observed
group. Bounds may be Python integers, dates, datetimes, or one-element non-null
Series. Typed Series preserve the exact integer dtype or Datetime unit and
timezone. Equal bounds return no gaps. Reversed bounds are invalid.

All original rows are validated before clipping, including rows outside the
domain and calls with an empty domain. Gaps include uncovered leading and
trailing regions. There are no inferred infinite gaps.

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

Frame outputs contain `[requested keys..., "start", "end"]`, even with custom
input endpoint names. Endpoints keep their exact logical dtypes and temporal
metadata, including when no segments remain. Groups appear in first-observed
input order before clipping or empty-row removal. Segments inside each group
are start-sorted, nonempty, and strictly separated.

`by=None` and `by=[]` mean one collection. Group keys support String, Boolean,
8/16/32/64-bit integers, Date, and Datetime. Null key values group together.
Duplicate or missing keys, unsupported key dtypes, and keys named `start` or
`end` are rejected. Other input columns are ignored. The expression's `.over`
uses Polars' grouping behavior.

An absent key has no output. To request gaps for resources absent from the
input would require a separate domain-table feature, which is outside this API.

## Eager and lazy execution

Frame functions return the same frame kind they receive. This pipeline remains
lazy until `collect`:

```python
query = (
    grouped.lazy()
    .pipe(pi.interval_gaps, by="resource", domain_start=0, domain_end=16)
    .filter(pl.col("start") >= 8)
    .select("resource", "start", "end")
)
assert isinstance(query, pl.LazyFrame)
result = query.collect(engine="streaming")
```

Building a plan, explaining it, and resolving its schema do not execute input
rows. Schema errors may appear during planning. Row errors appear at execution
and use indices in the original evaluated input at the operation boundary.

Clustering is a non-elementwise expression over the complete collection or
group. Union and gaps use a blocking native adapter. Their lazy nodes declare
the output schema, validate it, and prevent filter, projection, and slice
pushdown across the operation. Required input columns are selected upstream.
Native grouping, solving, and output assembly release the Python GIL.

All chunks and scan batches form one instance. The streaming engine uses the
same blocking boundary. These APIs materialize their input at that boundary
and do not promise bounded-memory streaming. Filtering before an operation
changes the interval instance. Filtering after it selects computed results.
Polars may eliminate a wholly unused node, in which case it need not validate.

## Exactness and cost

Endpoints follow the shared [input rules](usage.md#interval-rules). They must be
matching integer, Date, or Datetime columns with no nulls or reversed rows.
There are no implicit casts or floating-point intermediates. Kernels compare
endpoints without subtraction, so extreme UInt64 values and physical timestamps
remain exact. `include_touching` requires a real Boolean. Cluster ID overflow
raises an error rather than wrapping.

After sorting nonempty intervals by start, a scan maintains the furthest end of
the current run. A new interval belongs to that run precisely when its start is
before that end for strict clustering, or at most that end for touching and
union. The furthest-reaching earlier interval provides the needed connection.
When the condition fails, no earlier interval can connect to this one or to a
later interval across the boundary. This proves the connected components.
Empty rows are handled separately. A pass in original row order assigns the
canonical component IDs.

The same touching-inclusive frontier gives maximal exact union ranges. Every
range is covered, and every positive gap separates two ranges. Gaps advance
through the clipped union inside the explicit domain and emit exactly the
uncovered stretches before, between, and after its ranges.

General kernel cost is O(n log n + z) time and O(n + z) space. Here z is n for
clustering and the number of output segments for union or gaps. Verified start
order permits a linear scan directly from the original buffers. This path
allocates only the output labels or ranges. Empty rows do not affect the order
check. For gaps, the check considers clipped nonempty contributions. The native
Polars adapter can still allocate for chunk extraction, grouping, and output
assembly. No cluster labels are allocated for union or gaps. Separate public
calls prepare their own inputs.

See the [benchmark report](interval-geometry-benchmarks.md) for production choices,
measured comparisons, memory definitions, and limits. Independent pair-graph,
elementary-cell, and bounded-integer bitmap oracles check the kernels. To run a
larger property suite locally:

```powershell
$env:PROPTEST_CASES = "2048"
cargo test -p intervals-core --test geometry --locked
Remove-Item Env:PROPTEST_CASES
```
