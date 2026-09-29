# Coverage and resource-load profiles

[`coverage_profile`](api.md#polars_intervals.coverage_profile) turns intervals
into new segments describing how much is active at each coordinate. Use it for
genomic coverage depth, concurrent resource demand, or zero-load periods inside
an observation horizon. A DataFrame input returns a DataFrame. A LazyFrame input
returns a LazyFrame that computes the same profile when collected.

## Coverage depth

Each interval contributes one unit unless you explicitly name a weight column.
Columns called `weight` or `load` are otherwise ignored.

```python
import polars as pl
import polars_intervals as pi

reads = pl.DataFrame({"start": [0, 2, 5], "end": [4, 5, 7]})
depth = pi.coverage_profile(reads)
print(depth.rows())  # [(0, 2, 1), (2, 4, 2), (4, 7, 1)]
```

An interval `[s, e)` contributes at `s` and stops at `e`. Empty `[x, x)` rows
contribute nothing. Duplicate nonempty rows count independently. The boundary
at 5 disappears because the load stays at one even though the active read changes.
Touching segments with equal loads always coalesce. Equal loads separated by
an omitted positive-length gap remain separate segments.

Coverage depth describes coordinates. `overlap_count` instead returns, for each
input row, how many other rows intersect it. `max_weight_clique` selects a set
of intersecting rows. These answer different questions and have different
empty-row conventions.

## Resource demand

An explicit weight is the constant quantity used throughout an interval. It is
neither a scheduling profit nor an amount divided by the interval's duration.

```python
jobs = reads.with_columns(pl.Series("demand", [2, 3, 3]))
load = pi.coverage_profile(jobs, weight="demand")
print(load.rows())  # [(0, 2, 2), (2, 4, 5), (4, 7, 3)]
```

For every coordinate `t`, the function is exactly the sum of quantities of rows
with `start <= t < end`. Quantities must be nonnegative signed or unsigned
8-, 16-, 32-, or 64-bit integers. Zero is valid. Floating-point, Boolean,
Decimal, temporal and Int128 input weights are rejected. Omitted weights use
a dedicated unit path, without constructing a column of ones.

## Observation horizons and gaps

By default, each group's domain is the hull of **all nonempty input intervals**,
including zero-weight intervals. Empty intervals never extend that hull.
An empty input or an empty-only group has no inferred domain.

Pass both bounds to restrict the domain. Rows are validated first, then clipped.
Use `include_zero=True` to partition the entire domain, including uncovered gaps:

```python
observed = pi.coverage_profile(
    jobs, weight="demand", domain_start=-1, domain_end=8, include_zero=True
)
print(observed.rows())
# [(-1, 0, 0), (0, 2, 2), (2, 4, 5), (4, 7, 3), (7, 8, 0)]
```

The default `include_zero=False` omits zero-load segments. Full mode has no
infinite tail: it stops at the inferred or supplied bounds. All-zero nonempty
inputs produce their hull as one zero segment in full mode. An explicit
nonempty domain produces one zero segment for an ungrouped empty input in full
mode. Equal domain bounds produce no rows, after validation.

Bounds follow the same scalar rules as `minimum_cover`: Python integers must
fit the endpoint dtype, and typed one-element Series must match its dtype
exactly. Date and Datetime scalars preserve their logical metadata. For exact
nanoseconds, units or timezone metadata, supply typed singleton Series:

```python
timestamp_type = pl.Datetime("ns", "Europe/Helsinki")
timestamps = pl.DataFrame({"start": [2**53 + 1], "end": [2**53 + 5]}).cast(timestamp_type)
timed = pi.coverage_profile(
    timestamps,
    domain_start=pl.Series([2**53 + 2], dtype=timestamp_type),
    domain_end=pl.Series([2**53 + 4], dtype=timestamp_type),
)
```

There is no timezone/unit conversion, endpoint subtraction, or floating-point
rounding. Timestamp order means physical chronological order, including across
timezone offset transitions.

## Independent groups

```python
chromosomes = reads.with_columns(pl.Series("chromosome", ["chr2", "chr1", "chr2"]))
per_chromosome = pi.coverage_profile(chromosomes, by="chromosome")
print(per_chromosome.rows())
# [('chr2', 0, 4, 1), ('chr2', 5, 7, 1), ('chr1', 2, 5, 1)]
```

`by` accepts a name or an ordered list of distinct names. `None` and `[]` mean
one ungrouped collection. Keys may be String, Boolean, integer up to 64 bits,
Date or Datetime. Null tuples form ordinary groups and are retained. Unsupported
key types are rejected. Keys cannot be named `start`, `end` or `load`, because
those names are reserved for output.

Groups appear in first-appearance order, with segments sorted by start inside
each group. All chunks participate in the same solve per key. Only observed key
tuples exist: a grouped empty frame has no output, even with explicit bounds.
An observed group containing only empty rows can produce a zero segment with
explicit bounds and full mode. Bounds are shared across all groups.

## Schema, validation and execution

Output columns are `[requested group keys..., "start", "end", "load"]`, even
when source columns have other names. Key and endpoint dtypes are preserved,
including temporal units/timezones. Endpoints must have identical integer
8/16/32/64-bit, Date or Datetime dtypes. `load` is always non-null `pl.Int128`,
including unit mode and typed empty results. New endpoints contain no nulls.

Null endpoints/weights, reversed intervals, negative weights, invalid bounds
and unsupported types are errors. When the operation executes, every row is
validated, including empty, zero-weight and clipped-away rows. Row diagnostics
refer to the rows entering this operation, after any upstream lazy filters or
sorts. Input frames are unchanged, and an error returns no partial result.

Loads use checked i128 addition. Only an unrepresentable load on a represented
positive-length segment is an overflow error. Departures occur before arrivals
at tied coordinates. The algorithm never sums total input weight or multiplies
load by duration. Those quantities can exceed i128 even when the profile fits.

## Eager and lazy execution

Pass a LazyFrame to keep the profile in a query plan. Input expressions run
before the profile, and downstream operations consume its new segment rows:

```python
query = (
    jobs.lazy()
    .with_columns((pl.col("demand") * 2).alias("demand"))
    .pipe(pi.coverage_profile, weight="demand")
    .filter(pl.col("load") >= 6)
)
assert isinstance(query, pl.LazyFrame)
print(query.collect().rows())  # [(2, 4, 10), (4, 7, 6)]
```

Constructing this query resolves its schema without computing the profile or
collecting its input rows. Column and option checks happen during construction.
Row validation and native domain checks happen when the profile executes.
`query.collect_schema()` already exposes the exact endpoint/key dtypes and
Int128 load dtype, including when execution will return no rows.

The lazy operation invokes the same Rust implementation once on the whole
collection, releasing the Python GIL for native work. All chunks and observed
groups participate together. It is a **blocking operation**, including with
`.collect(engine="streaming")`: surrounding query stages may stream, but the
profile needs its complete input in memory. Lazy support does not make profile
construction a streaming algorithm with bounded memory.

Filters, projections and slices after the profile cannot move through it and
change its input. Filters before it deliberately choose the intervals being
profiled. As with other lazy operations, Polars may eliminate computation whose
result is unused, such as a query ending in `.head(0)`. Row validation then does
not run. An executed profile validates its entire input even for an empty domain
or when subsequent operations retain only a few segments.

For an eager DataFrame, computation happens immediately and returns a DataFrame.
Call this function directly or with `.pipe()`. It returns a frame, so it cannot
be used as a column expression in `.with_columns()`.

The core validates in O(n), independently orders positive clipped starts/ends,
and emits canonical output while merging the two streams. With `m` contributing
rows and `z` segments, it takes O(n + m log m + z) time and O(m + z) core space.
Verified ordered streams take O(n + z). The complete binding also includes
column extraction, native grouping, gathering and key repetition, using
O(n + z) storage for a fixed number of keys. No allocation depends on the
coordinate span. See the [benchmarks](coverage-profile-benchmarks.md) for measured
tradeoffs with event sorting, heaps and native Polars expressions.

## Passing loads to other APIs

Existing APIs accepting at most 64-bit quantities require **explicit checked
narrowing** of the Int128 output. A failed cast must remain an error:

```python
bounded = load.with_columns(pl.col("load").cast(pl.UInt64, strict=True))
again = pi.coverage_profile(bounded, weight="load")
assert again.equals(load)
```

Do not narrow silently. For example, two overlapping `UInt64::MAX` demands
produce an exact load of `36893488147419103230`, which cannot be narrowed to
UInt64. This API constructs only the profile. Threshold selection, histograms,
integration and scheduling decisions are separate operations.
