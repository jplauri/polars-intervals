# Clustering, union, and gaps benchmarks { #interval-clustering-union-and-gap-benchmarks }

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

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

**What was compared**

The public package functions and one native Polars implementation solve the
same geometry problems. Clustering labels original rows, supports strict
overlap or touching, and keeps each empty row separate. Union returns maximal
nonempty covered runs; gaps return their complement inside a domain. Each
observed group is solved independently with exact endpoint types.

Native Polars sorts nonempty rows, computes grouped prefix maxima, finds
component boundaries and restores canonical outputs. It includes validation,
original-row order, observed-group ordering, isolated empty rows, null keys
and domain handling. Lazy validation uses a blocking node. Unlike the package
expression API, the comparator accepts a frame and column names; its
[implementation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/interval-geometry-notes.md)
explain that boundary. Speedup is native median divided by package median for
the same workload and size.

The headline selects small and large strict-clustering inputs, 100,000-row
touching/union/gap cases and timestamp groups from one broad eager run.
A separate million-row lazy run measured ordered-chain clustering at
**4.75 ms** package versus **65.2 ms** native, and nested union at **5.22 ms**
versus **45.2 ms**. Shuffling the chain raised package time to **44.7 ms**.

A two-seed repeat confirmed the small-group loss: at 1,000 singleton timestamp
groups, strict clustering took **10.3–10.4 ms** package versus **4.64–4.91 ms**
native. At 100,000 rows across the same 1,000 groups, package took
**14.5–14.9 ms** versus **26.0–27.1 ms**. These ranges span seed medians.

The separate 24-thread run narrowed large-case advantages. Million-row
timestamp union changed from **58.3 / 259 ms** package/native with one thread
to **50.7 / 83.6 ms** with 24. Shuffled clustering changed from
**44.7 / 311 ms** to **61.3 / 136 ms**. The package retained its selected
large-case advantage for both seeds; native clustering retained its
singleton-group win. Core scans are sequential; Polars query work can use
the configured pool.

A [separate phase run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-lazy-20260929.metadata.json)
measured ordered 100,000-row union planning at **0.0319 / 0.195 ms**
package/native and prebuilt-plan collection at **0.596 / 4.33 ms**.
Warm Parquet collection with eight groups took **7.71 / 24.4 ms** including
reads; complete streaming-engine calls took **7.86 / 29.7 ms**.

**Settings**

See shared [hardware](benchmarks.md#hardware) and the [measurement guide](benchmarking.md).
Broad eager runs used one thread, one warmup and three samples. Selected
million-row, thread, phase and small-group repeats used two warmups and five
samples. Tables select seed 7; repeated size/thread runs also preserve seed 41.
Method order rotates between samples. Exact scopes and settings remain separate:

- [Broad eager run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.metadata.json)
- [Million-row lazy run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.metadata.json)
- [24-thread lazy run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.metadata.json)
- [Million-row core run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.metadata.json)

Complete Python timings include argument checks, validation, extraction,
grouping, sorting, canonical remapping, output construction and native crossings.
Lazy complete calls also include planning and collection; the phase run reports
these separately. Inputs are already in memory except for the warm Parquet
cases. Fixture construction, compilation, correctness checks and Python output
destruction are excluded. Core calls include output destruction and do not
represent complete Polars times.

<span id="coverage-and-limitations"></span>

**Limitations**

The synthetic matrix spans 0–100,000 rows plus selected million-row cases.
It varies ordered/reversed/shuffled/partly ordered inputs, chains, nesting,
disjoint ranges, repeated endpoints, ties, sparse/empty rows, domains and large
coordinates. Complete Polars runs cover UInt64 above 2^53, Date, nanosecond
Datetime, null keys, few large/many small/skewed groups, differing chunks, wide
payloads and scans. Core runs cover i64/u64/i16, omitting out-of-range narrow
coordinates.

Small cases use independent overlap graphs, elementary-cell membership and
bounded integer bitmaps to check canonical labels and exact gaps. Large cases
compare complete outputs and structural invariants; they lack an independent
large-instance oracle. Every row is validated before clipping or fast paths.
Tests also cover pushdowns, schema-only planning, streaming, multiple keys and
exact temporal metadata.

Clustering returns one label per input row. Million-row nested union returns
one range; disjoint gaps return **1,000,001**. The narrow-domain eight-group case
returns no gaps. Timestamp fixtures have 1,000 observed groups including a null
key; the skewed fixture puts about 75% of rows in its largest group. Raw samples
record group sizes, clipped/nonempty rows, components and output counts.

Many singleton groups incur per-group expression overhead. Streaming still
needs the complete collection or group in memory. Other machines, Polars
versions and real workload distributions remain unmeasured. An optional
coverage-profile reconstruction baseline was omitted.

[Separate fresh-process measurements](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-memory-20260929.memory.json)
record Windows working set and peak working set with outputs retained.
Million-row shuffled clustering peaked at **160 MB** package versus **197 MB**
native; ordered clustering at **123 MB** versus **194 MB**. These process metrics
include inputs and the runtime. A prior fixture peak can mask later allocations.
They were collected separately from timing and differ from core requested-heap
metrics; see the [memory definitions](benchmarking.md#memory-metrics).

<span id="reproduce-and-data"></span>

**Reproduce**

Follow the [shared setup](benchmarking.md#setup) and rebuild the release plugin.
The runner verifies that the installed extension matches that release binary.
Use a new output prefix for every run. In PowerShell:

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

Exact headline and supplemental table downloads:

Headline cases:

--8<-- "docs/assets/benchmarks/geometry-summary.md:-2:"

Broad eager comparisons:

--8<-- "docs/assets/benchmarks/interval-geometry-polars-table.md:-2:"

Million-row lazy comparisons:

--8<-- "docs/assets/benchmarks/interval-geometry-million-table.md:-2:"

24-thread lazy comparisons:

--8<-- "docs/assets/benchmarks/interval-geometry-threads24-table.md:-2:"

Private Rust algorithm comparisons:

--8<-- "docs/assets/benchmarks/interval-geometry-core-table.md:-2:"

Raw samples, metadata and source archives:

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

**History**

The headline run was recorded at `2026-09-29T19:36:11.609785+00:00`, based on
`362aa5b2cf63d826dde98c28d72efac6ff29c9cf` with uncommitted feature sources.
The [source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.sources.zip)
declares version `0.2.0`; core and adapter match `43e9af1`. Later `83e8e44`
removes duplicate adapter type/length checks and replaces the native competitor's
Rust validator with Polars expressions. Both affect complete-query timing.
Shared predicates/conversions are another refactor; runner bookkeeping is
outside timing. Public contracts are unchanged. Saved timings and memory
measurements do not measure this cleanup.

All final runs report unchanged measured sources. Complete Polars measurements
used the verified release extension SHA-256
`be5e9721282db7d605d1b6dfbf4b0226254f2bd4ae6ed83af70a162b023cc7bc`.
Dirty-tree archives identify the measured feature sources. Initial packed and
direct-buffer prototype evidence remains in the
[comparison notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/interval-geometry-notes.md).

Verified ordered input lets production scan endpoint buffers directly.
Unsorted input uses packed records; private indexed candidates read endpoints
indirectly. Both validate and prepare inputs before producing complete outputs.
For gaps, only clipped survivors affect the order check.

In the million-row core comparison, packed shuffled disjoint clustering took
**53.4 ms** versus indices **69.3 ms**, but partly ordered spanning input favored
indices: **31.4 ms** versus packed **38.1 ms**, for both seeds. An explicit order
check alone did not establish a gain on sorted disjoint inputs: private packed
clustering took **20.9 ms** with it versus **20.5 ms** with unconditional sorting.
Production's direct-buffer call took **3.66 ms**.

Fused gaps took **38.4 ms** for shuffled disjoint inputs versus **45.8 ms** for
union then complement. Both requested **33.6 MB** peak heap; fused emission made
38 allocations versus 57. For nested sorted rows, production gaps took
**3.05 ms** versus the materialized route's **11.5 ms**. Preparation is timed.

Core memory is **peak requested live heap in a separate untimed call**,
including output and reallocations but excluding input buffers, stack,
allocator overhead and process RSS. Sorted clustering requested **4.00 MB** for
million-row UInt32 labels. Shuffled disjoint clustering requested **36.4 MB**
packed versus **20.8 MB** indices; partly ordered spanning input requested
**28.0 MB** versus **12.4 MB**. The unsorted speed choice has a substantial
memory cost. Each case's memory probe is repeated alongside timing rows,
rather than measured anew per sample.

</details>
