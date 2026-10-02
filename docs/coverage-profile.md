# Coverage and load profiles { #coverage-and-resource-load-profiles }

[`coverage_profile`](api.md#polars_intervals.coverage_profile) turns intervals
into new segments describing how much is active at each coordinate. Use it for
genomic coverage depth, concurrent resource demand, or zero-load periods inside
an observation horizon.

## Coverage depth

Without `weight`, each interval contributes one unit.
Columns called `weight` or `load` are otherwise ignored.

```python
import polars as pl
import polars_intervals as pi

reads = pl.DataFrame({"start": [0, 2, 5], "end": [4, 5, 7]})
depth = pi.coverage_profile(reads)
print(depth.rows())  # [(0, 2, 1), (2, 4, 2), (4, 7, 1)]
```

Empty intervals contribute nothing. Duplicate nonempty rows count independently.
The boundary at 5 disappears because the load stays at one even though the active
read changes. Touching segments with equal loads merge. Equal loads separated by
an uncovered gap remain separate segments.

Coverage depth describes coordinates. To count overlaps for each input row, use
[`overlap_count`](usage.md#count-overlaps). To measure covered length inside each
reporting window, use [`coverage_stats`](coverage-stats.md).

## Resource demand

Use `weight` for a constant quantity used throughout an interval, such as the
number of machines a job needs.

```python
jobs = reads.with_columns(pl.Series("demand", [2, 3, 3]))
load = pi.coverage_profile(jobs, weight="demand")
print(load.rows())  # [(0, 2, 2), (2, 4, 5), (4, 7, 3)]
```

At each coordinate, load is the sum of quantities used by active intervals.
See the shared [weight and cost rules](usage.md#weights-costs-and-other-values)
for accepted types and values.

## Observation horizons and gaps

By default, each group's domain runs from its earliest nonempty start to its
latest nonempty end, including zero-weight intervals. Empty intervals do not
extend the domain. An empty input or a group with only empty intervals has no
inferred domain.

Pass `domain_start` and `domain_end` to set an observation horizon. Rows are
validated first, then clipped. Use `include_zero=True` to include uncovered gaps
across the entire domain:

```python
observed = pi.coverage_profile(
    jobs, weight="demand", domain_start=-1, domain_end=8, include_zero=True
)
print(observed.rows())
# [(-1, 0, 0), (0, 2, 2), (2, 4, 5), (4, 7, 3), (7, 8, 0)]
```

The default `include_zero=False` omits zero-load segments. With
`include_zero=True`, output covers the inferred or supplied domain. All-zero
nonempty inputs produce one zero-load segment across their domain. An ungrouped
empty input produces one zero-load segment when given nonempty bounds. Equal
bounds produce no rows. Reversed bounds are invalid.

Bounds follow the scalar rules of
[`minimum_cover`](api.md#polars_intervals.minimum_cover). Python integers must
fit the endpoint dtype. Typed one-element Series must match its dtype exactly.
Use typed Series for exact nanoseconds, time units or timezone metadata:

```python
timestamp_type = pl.Datetime("ns", "Europe/Helsinki")
timestamps = pl.DataFrame({"start": [2**53 + 1], "end": [2**53 + 5]}).cast(timestamp_type)
timed = pi.coverage_profile(
    timestamps,
    domain_start=pl.Series([2**53 + 2], dtype=timestamp_type),
    domain_end=pl.Series([2**53 + 4], dtype=timestamp_type),
)
```

Endpoints retain their exact types, time units and timezones.

## Independent groups

```python
chromosomes = reads.with_columns(pl.Series("chromosome", ["chr2", "chr1", "chr2"]))
per_chromosome = pi.coverage_profile(chromosomes, by="chromosome")
print(per_chromosome.rows())
# [('chr2', 0, 4, 1), ('chr2', 5, 7, 1), ('chr1', 2, 5, 1)]
```

Groups appear in first-appearance order, with segments sorted by start within
each group. See the shared [frame rules](usage.md#frame-results) for supported
group keys. Here, `start`, `end` and `load` are reserved output names.

Bounds apply to every group present in the input. A group containing only empty
intervals produces a zero-load segment with explicit nonempty bounds and
`include_zero=True`. A grouped empty frame produces no rows.

## Output and validation

Output columns are `[requested group keys..., "start", "end", "load"]`, even
when source columns have other names. `load` is always non-null `pl.Int128`,
including unweighted and empty results. See the shared
[interval rules](usage.md#interval-rules) for endpoint requirements.

Every input row is validated, including empty, zero-weight and clipped-away
rows. Error row indices refer to the input at this operation, after any earlier
filters or sorts. A segment load that exceeds `Int128` raises an error.

## Lazy queries

Pass a LazyFrame to keep the profile in a query plan. Input expressions run
before the profile, and downstream operations consume its new segment rows:

```python
query = (
    jobs.lazy()
    .with_columns((pl.col("demand") * 2).alias("demand"))
    .pipe(pi.coverage_profile, weight="demand")
    .filter(pl.col("load") >= 6)
)
print(query.collect().rows())  # [(2, 4, 10), (4, 7, 6)]
```

Filters before the profile choose which intervals contribute. Filters after it
choose which computed segments to keep. Like the other
[frame operations](usage.md#frame-results), the profile needs its complete input
in memory, including when collected with the streaming engine.

## Passing loads to other APIs

To reuse these `Int128` loads as 64-bit quantities, cast with `strict=True`:

```python
bounded = load.with_columns(pl.col("load").cast(pl.UInt64, strict=True))
again = pi.coverage_profile(bounded, weight="load")
assert again.equals(load)
```

For example, two overlapping maximum `UInt64` demands produce a load of
`36893488147419103230`. That value cannot fit in `UInt64`, so the cast raises an
error.

See the [API reference](api.md#polars_intervals.coverage_profile) for the full
contract and the [benchmarks](coverage-profile-benchmarks.md) for runtime, memory
use and algorithm details.
