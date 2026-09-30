# Interval clustering, union and gap benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`cluster_intervals`](api.md#polars_intervals.cluster_intervals) labels
connected intervals,
[`merge_intervals`](api.md#polars_intervals.merge_intervals) combines covered
ranges, and [`interval_gaps`](api.md#polars_intervals.interval_gaps) finds
uncovered ranges within a domain. In synthetic benchmarks, clustering **100,000
shuffled intervals took 4.11 ms**, **7.5× faster** than the native Polars
implementation tested, with Polars restricted to one thread. Merging and finding
gaps in 100,000 separate intervals took **4.16–4.28 ms**, **about 10× faster**
under the same thread restriction. Native clustering won on many tiny groups.

## Results

**Full Polars query time · milliseconds**

Native Polars uses sorted endpoints and cumulative maximums to build the same
labels, merged ranges or gaps.

--8<-- "docs/assets/benchmarks/geometry-summary.md:3:-3"

Grouping adds overhead when each group contains only one interval. For 1,000
singleton timestamp groups, package clustering took 10.3 ms, **2.2× slower**
than native Polars.

More cores narrow some advantages. Measurements used a September 29, 2026
build labeled 0.2.0.
Input checks changed afterward in both implementations, and those changes
have not been timed.

<details markdown="1">
<summary>Benchmark details</summary>

**Measured build**

The headline run was recorded at `2026-09-29T19:36:11.609785+00:00`, with
base revision `362aa5b2cf63d826dde98c28d72efac6ff29c9cf` and uncommitted feature
sources. The [archived source](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.sources.zip)
declares package version `0.2.0`; its production core and adapter match `43e9af1`.
The later `83e8e44` refactor removes duplicate adapter type/length checks and
replaces the native competitor's Rust validator with Polars expressions.
Both affect complete-query timing paths. Sharing predicates and conversion
helpers is a separate refactor, while runner provenance bookkeeping is outside
timing. The archived plugin identity and source hashes remain in the metadata.

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and case selection**

The headline table selects small and large strict-clustering inputs,
100,000-row cases for touching, union and gaps, and timestamp groups where
native clustering can win. All headline rows come from the same broad eager run.
The displayed times are medians from three samples after one warmup, with
one Polars thread and seed 7.

The benchmark's native implementation is the only native candidate measured in
this run. Speedup is native median divided by package median. The loss uses
package median divided by native median.

--8<-- "docs/assets/benchmarks/geometry-summary.md:-2:"

**Operation and earlier measurements**

Clustering returns labels in original row order. It supports strict overlap or
touching connectivity, and each empty row remains a separate component. Merging
returns the exact union as maximal nonempty runs. Gaps return its complement
inside a supplied domain. These operations preserve exact endpoint types and
solve each observed group independently.

For **one million synthetic ordered intervals**, a complete lazy call took
**4.75 ms** to label an overlapping chain and **5.22 ms** to merge nested
intervals. A native Polars implementation took **65.2 ms** and **45.2 ms**
respectively (one thread, seed 7, median of five samples). Shuffling the chain
raised package clustering time to **44.7 ms**. Verified input order permits
direct scans of the original endpoint buffers. Other inputs use packed records.

Many tiny groups are a meaningful exception. For **1,000 singleton timestamp
groups**, native Polars clustering took **4.73 ms** eagerly against the package's
**10.3 ms**. More Polars threads narrow several large-input differences too.
The measurements support a simple common core, with no workload thresholds or
claim that it is universally fastest.

Saved timings and process-memory measurements describe the archived versions.
Later cleanup shared core/adapter helpers and benchmark bookkeeping, removed
redundant validation calls, and replaced the competitor's Rust validator with
Polars checks. Public contracts are unchanged. These tables do not measure the
refactored code, and no timing or memory improvement is claimed for the cleanup.

**Broad Polars comparisons**

Complete eager Polars calls · one thread · seed 7 · median of three samples ·
[settings and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/interval-geometry-polars-table.md"

The package column calls the public API. The private native competitor sorts
nonempty rows, computes grouped prefix maxima of ends, identifies component
boundaries, and assembles canonical outputs with Polars expressions. It includes
validation, original-row restoration, observed-group ordering, isolated empty
rows, nullable keys and bounded-domain handling. Lazy validation uses a blocking
node before the native geometry plan. The comparator takes a frame and column
names, rather than exposing the public clustering function's arbitrary-expression
API. Its [implementation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/interval-geometry-notes.md)
describe this boundary.

The small-group loss persisted in a separate five-sample, two-seed repeat:
strict clustering took **10.3–10.4 ms** for the package and **4.64–4.91 ms** for
native Polars at 1,000 rows. At 100,000 rows across the same 1,000 groups, those
medians were **14.5–14.9 ms** and **26.0–27.1 ms**. These ranges span seed medians,
not individual samples. Per-group expression invocation is costly when each
group has only one row. Replacing the expression API with a frame query would
change its integration contract, so this remains a documented limitation.

Complete lazy calls including planning and collection · one thread · seed 7 ·
median of five samples ·
[million-row settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/interval-geometry-million-table.md"

Output size changes substantially across these examples. The nested union has
one output row, the disjoint gap case has **1,000,001**, and the narrow-domain
eight-group case has none because every group's domain is fully covered.
Clustering always returns one label per input row. Timestamp cases have 1,000
observed groups including a null-key group. The skewed fixture below puts about
75% of rows in its largest group. Raw samples record observed group sizes,
nonempty and clipped rows, components or segments, and output counts.

Complete lazy calls including planning and collection · 24 Polars threads ·
seed 7 · median of five samples ·
[thread comparison settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/interval-geometry-threads24-table.md"

The native million-row timestamp union improves from **259 ms** with one thread
to **83.6 ms** with 24. The package changes from **58.3 ms** to **50.7 ms**.
Package shuffled clustering is slower in the 24-thread run, **61.3 ms** versus
**44.7 ms**, while native clustering improves from **311 ms** to **136 ms**.
These are separate runs, each with its own samples. The package remains faster
on the selected large cases for both seeds, while native clustering retains
its singleton-group win. Core scans are sequential. Polars grouping and query
work can use the configured thread pool.

A [separate phase run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-lazy-20260929.metadata.json)
distinguishes plan construction from execution. For 100,000 ordered rows,
union planning took **0.0319 ms** package versus **0.195 ms** native. Collecting
the prebuilt plans took **0.596 ms** versus **4.33 ms**. For a warm Parquet scan
with eight groups, prebuilt collection took **7.71 ms** versus **24.4 ms**,
including file reads. Complete streaming-engine calls took **7.86 ms** versus
**29.7 ms**. These medians use five samples and one thread. Both interval nodes
need the complete collection or group. The streaming label does not establish
bounded memory or out-of-core execution.

**Rust algorithm comparison and production decision**

Complete Rust calls including output destruction · Int64 endpoints · sequential · seed 7 ·
median of five samples ·
[final production settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/interval-geometry-core-table.md"

Packed records carry endpoints and, for clustering, original row indices.
The indexed candidate sorts row indices and reads endpoints indirectly.
Both private table candidates validate, prepare, check start order, sort when
needed, scan and build complete outputs. The public production calls additionally
avoid either temporary representation when relevant starts are already ordered.
Validation still visits every original row. For gaps, the order check considers
clipped survivors, so irrelevant outside-domain rows do not disable that path.

The packed fallback is faster than indices on shuffled disjoint clustering,
**53.4 ms** versus **69.3 ms**, but loses on partly ordered spanning intervals:
**38.1 ms** versus **31.4 ms**. Both seeds retain this loss. We keep contiguous
records for unsorted input and the direct scan for verified order. No size- or
distribution-based dispatcher is introduced.

The raw data also compare explicit order checks with unconditional sorting.
For a million sorted disjoint rows, private packed clustering took **20.9 ms**
with the check and **20.5 ms** with unconditional sorting. The important saving
is the production direct-buffer call at **3.66 ms**, not an assumption that a
pre-sort check alone beats the standard library's handling of ordered input.

Fused gaps avoid a separate union output. With the same private packed
preparation, a million shuffled disjoint rows took **38.4 ms** fused versus
**45.8 ms** for union then complement. Both had **33.6 MB** peak requested heap
because their largest live buffers had similar capacity. Materialization made
57 allocations versus 38 for fused emission. On nested sorted rows, production
gaps took **3.05 ms**, with only output storage, against **11.5 ms** for the
materialized route. These comparisons include all preparation.

Core memory means **peak requested live heap in a separate untimed call**.
It includes output and reallocations, and excludes input buffers, stack,
allocator overhead and process RSS. Sorted clustering requested **4.00 MB**
for its million UInt32 labels. Shuffled disjoint clustering requested
**36.4 MB**, versus **20.8 MB** for indices. Partly ordered spanning clustering
requested **28.0 MB**, versus **12.4 MB** for indices. The unsorted speed choice
therefore has a substantial memory cost. Each case's single memory probe is
repeated alongside timing rows in the raw CSV, not measured anew per sample.

Initial packed-production and direct-buffer-prototype measurements are retained
as historical evidence in the [comparison notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/interval-geometry-notes.md).
Those runs measured the public implementation after a fresh rebuild, before the
later cleanup described above.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

The [single-machine](benchmarks.md#hardware) synthetic matrix includes 0, 8,
1,000, 10,000 and 100,000 rows, plus selected million-row cases. It varies
ordered, reversed, shuffled and partly ordered inputs, nesting, chains,
disjoint intervals, repeated endpoints, ties, sparse and empty rows, domain
selectivity and large coordinate magnitudes. Complete Polars runs cover
UInt64 above 2^53, Date, nanosecond Datetime, null keys, few large and many small
groups, skew, mismatched chunks, wide payloads and scan-backed lazy inputs.
Core runs include i64, u64 and i16, omitting coordinates outside the narrow
type's range. The broader runs use one warmup and three samples. Selected
million-row, thread, phase and small-group repeat runs use two warmups and five
samples. Tables use seed 7. The latter repeated size/thread runs also retain
seed 41 separately. Method order rotates between samples.

Small cases use independent pairwise overlap graphs and direct membership on
elementary coordinate cells or bounded integer bitmaps. These check the original
problem, including canonical labels and exact gaps. Large benchmark cases compare
full outputs and structural invariants between implementations. They do not
have an independent large-instance oracle. Shared validation checks invalid
rows before clipping and fast paths. Tests also exercise optimizer pushdowns,
schema-only lazy planning, streaming, multiple keys and exact temporal metadata.

Python timings include argument checks, validation, extraction, grouping,
sorting, canonical remapping, output construction and native crossings.
Input construction, compilation, correctness checks and Python output destruction
are excluded. Core timings cannot stand in for complete Polars times.
Parquet measurements use warm files. Hardware variation, other Polars versions
and real workload distributions remain unmeasured. The optional coverage-profile
reconstruction baseline was omitted. No geometry operation needs a profile.

[Separate fresh-process measurements](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-memory-20260929.memory.json)
record Windows working set and peak working set while retaining outputs.
For million-row shuffled clustering, peak process working set was **160 MB**
package versus **197 MB** native. Ordered clustering peaked at **123 MB**
versus **194 MB**. These include input buffers and the runtime, and are not
algorithm allocation counts. A prior fixture peak can mask later allocations.
They were collected separately from timing. See the shared
[memory definitions](benchmarking.md#memory-metrics).

<span id="reproduce-and-data"></span>

**Reproduce and data**

Follow the [shared setup](benchmarking.md#setup) and rebuild the release plugin.
The complete-call runner verifies that the installed extension matches that
release binary. Use a new output prefix for every run. In PowerShell:

```powershell
$env:POLARS_MAX_THREADS = "1"
uv run --no-sync python -m benchmarks.interval_geometry --output benchmarks/results/geometry-new --sizes 0 8 1000 10000 100000 --samples 3 --warmups 1 --scopes eager lazy_complete
uv run --no-sync python -m benchmarks.interval_geometry --output benchmarks/results/geometry-million-new --sizes 1000000 --cases 0 1 2 3 7 8 14 --seeds 7 41 --samples 5 --warmups 2 --scopes eager lazy_complete
uv run --no-sync python -m benchmarks.interval_geometry_core --output benchmarks/results/geometry-core-new --sizes 0,8,1000,10000,100000 --samples 3 --warmups 1
```

For focused thread, phase and memory repeats, use the exact settings recorded
below. Set `POLARS_MAX_THREADS` before starting the process. Add `--memory` for
separate fresh-process workers. The core wrapper records Cargo release settings
and saves its source snapshot. Both runners preserve raw samples and reject
existing output prefixes. Generate tables with the isolated plots environment:

```powershell
uv run --locked --isolated --only-group plots python benchmarks/plot.py
```

Each final run has raw samples, metadata and an archived source snapshot:

| Run | Samples | Metadata | Sources |
| --- | --- | --- | --- |
| Broad Polars | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.sources.zip) |
| Million-row Polars | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.sources.zip) |
| 24 threads | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.sources.zip) |
| Planning and collection | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-lazy-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-lazy-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-lazy-20260929.sources.zip) |
| Small-group repeat | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-repeat-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-repeat-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-repeat-20260929.sources.zip) |
| Process memory | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-memory-20260929.memory.json) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-memory-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-memory-20260929.sources.zip) |
| Broad final core | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-final-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-final-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-final-20260929.sources.zip) |
| Million-row final core | [CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.csv) | [JSON](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.metadata.json) | [ZIP](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.sources.zip) |

All final runs report unchanged measured sources. Complete Polars measurements
used the verified release extension SHA-256
`be5e9721282db7d605d1b6dfbf4b0226254f2bd4ae6ed83af70a162b023cc7bc`.
The dirty-tree snapshots identify the exact measured feature sources. Later
documentation and packaging work does not relabel those binaries or samples.

</details>
