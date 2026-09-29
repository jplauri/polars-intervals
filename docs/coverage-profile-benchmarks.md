# Coverage and load profile benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`coverage_profile`](api.md#polars_intervals.coverage_profile) returns new segments
describing how many intervals are active, or their total resource demand, at
each coordinate. It coalesces touching equal loads, optionally includes zero
gaps, clips to a supplied observation horizon, and solves observed groups
independently. These are exact profiles, not selected subsets of input rows.

For **100,000 synthetic ordered short intervals**, the updated API took
**1.24 ms** eagerly, **1.58 ms** including lazy plan construction and collection,
and **1.71 ms** with streaming-engine collection (one thread, seed 7). Native
Polars events took **41.0 ms** in the same run. The retained Rust implementation
uses two independently sorted endpoint streams. It preserves exact Int128 loads
and canonical segments, with linear processing when both streams are ordered.
The choice trades higher weighted-record memory for faster large shuffled calls;
an active-end heap wins on some ordered weighted core workloads.

The timing tables below predate removal of the Rust core's explicit sortedness
guards. A [focused before/after follow-up](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-sort-review-20260929.md)
retains the simpler standard-library calls without claiming a speedup. Outputs
and allocations matched, while timing differences varied across repeat runs.
No new end-to-end Polars timings were taken for that cleanup.

## Results

Complete calls including query construction and collection · one Polars thread ·
median of five samples ·
[current comparison settings and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/coverage-profile-lazy-table.md"

Lazy input defers the same native engine until collection. Both engines process
the complete collection at the profile node; the streaming-engine label does
not mean profile construction has bounded memory. This run includes schema
resolution, plan construction and collection, with inputs already in memory.
For eight-row fixtures the extra scheduling work matters: eager calls took
roughly **0.05–0.10 ms**, versus **0.14–0.26 ms** lazy and **0.23–0.46 ms** with
the streaming engine (seed 7). Small apparent lazy wins in individual cases
are timing noise. These are convenience and query composition benefits, not
a claim of faster standalone execution.

This comparison uses the simplified native baseline, which attaches group and
domain metadata in one join. A [separate current million-row run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-million-20260929.metadata.json)
measured 32 clipped resources at **90.8 ms** eager, **90.9 ms** lazy and
**91.9 ms** streaming-engine collection, versus **302 ms** native. For 1,000
timestamp groups, the same calls took **77.6 / 78.6 / 78.8 ms**, versus
**1,020 ms** native (seed 7). Both seeds retained the package advantage.

The broader eager comparisons below predate the lazy wrapper and the native
baseline's join simplification. Their Python
source snapshots are preserved and their timings are not relabeled as the
updated wrapper. The Rust engine was unchanged in that follow-up, which measures
the eager and lazy routes against the simplified baseline. Those measurements
predate the sort-guard cleanup described above. The original lazy follow-up
also retains its earlier baseline source
snapshot; none of those saved timings are attributed to the changed baseline.
Native count comparisons remain available in the current raw samples.

Complete eager Polars calls · one Polars thread · median of five samples ·
[settings and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/coverage-profile-polars-table.md"

The package column calls the public function. The native-Polars competitors
validate input, build clipped endpoint tables with domain edges, aggregate by
group and coordinate, sort, cumulatively sum signed Int128 deltas, find the next
coordinate, and coalesce equal neighboring loads. The count variant aggregates
unit arrivals/departures with counts instead of constructing unit deltas. Both
include query planning, execution, group ordering and key assembly. On this
pinned Polars build, signed subtraction supplies departures because Int128
unary negation is unsupported.

The million-row run is separate from the smaller-size run:

Complete eager Polars calls · one Polars thread · median of five samples ·
[million-row settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-million-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/coverage-profile-million-table.md"

With one million short intervals, input order changed the package time from
**12.3 ms** ordered to **45.5 ms** shuffled, despite the same 15 output segments.
One million nested intervals produced **1,999,999 segments** in **53.7 ms**.
For repeated dates, native count aggregation improved on native event sums
(**90.1 ms** versus **118 ms**), while the package took **26.1 ms**. These are
seed-7 medians from the separate million-row run.

The output geometry matters as much as input size. All rows below have
`n = 1,000,000`, with seed 7. `m` counts positive clipped intervals, `u` counts
their distinct coordinates plus domain edges, `z` counts output segments, and
peak counts concurrently active positive rows (independently of their weights).
For grouped cases, coordinate counts are summed across groups and peak is the
largest group's concurrency.

| Synthetic workload | m | u | z | Peak rows |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, units | 1,000,000 | 1,000,008 | 15 | 8 |
| Variable durations, weighted | 956,521 | 998,362 | 957,190 | 246 |
| All overlap, weighted | 956,521 | 2 | 1 | 956,521 |
| Nested, units | 1,000,000 | 2,000,000 | 1,999,999 | 1,000,000 |
| Repeated dates, units, full mode | 1,000,000 | 31,250 | 31,249 | 64 |
| 32 resources, clipped, weighted | 246,944 | 249,568 | 239,328 | 246 |
| 1,000 timestamp groups, units | 1,000,000 | 1,008,000 | 15,000 | 8 |

More Polars threads substantially improve the native competitor on large
event tables. A separate run with **24 threads** reduced its shuffled short
interval median to **237 ms**, versus **50.9 ms** for the package. The closest
case was 32 clipped resource groups: **120 ms** native versus **95.6 ms** package.
Both seeds retained the package advantage in these six measured cases, but
the gap is smaller than the one-thread results suggest. The core sweep stays
sequential; grouping and other Polars work can use the configured pool.

Complete eager Polars calls · 24 Polars threads · median of five samples ·
[thread comparison settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-threads24-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/coverage-profile-threads24-table.md"

<details markdown="1">
<summary>Rust algorithm comparison and production decision</summary>

Complete Rust calls · sequential, one algorithm thread · median of five samples ·
[final production settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-final-20260929.metadata.json)

--8<-- "docs/assets/benchmarks/coverage-profile-core-table.md"

**A: events** sorts compact `(coordinate, tagged row index)` records, with
departures before arrivals. **B: package streams** independently sorts starts
and ends, then merges both streams. It uses endpoint-only arrays for units and
natural-alignment `(coordinate, quantity)` records for weights. **C: heap**
sorts/validates start order and maintains an active-end min-heap, processing
departures between starts and after the last start. **B: weighted indices**
stores two index arrays and reads caller endpoints and quantities indirectly.
Each candidate validates and clips before accumulating, emits the full profile,
and coalesces canonically. No sorting or preparation is supplied for free.

The retained production engine is B. Each stream uses the standard library sort,
which detects ordered inputs in linear time on the pinned Rust toolchain.
Ordered streams have linear work, including validation and
output. Units do not allocate a quantity vector. Weighted records were selected
for repeatable gains on large shuffled inputs, accepting higher allocation and
losses on some ordered or repeated-coordinate cases. There are no size thresholds
or machine-specific dispatch rules. Alternative engines remain private to the
benchmark/test harness.

In the final million-row shuffled weighted booking case, B took **131 ms**,
events **135 ms**, the heap **213 ms**, and indices **254 ms** (seed 7). The small
event difference alone does not establish a reliable winner. Both seeds show
the larger index-access penalty. The important loss is ordered weighted
bookings: B took **94.1 ms**, versus **69.4 ms** for the heap and **75.0 ms** for
indices. At 100,000 repeated endpoints, indices also won, **2.59 ms** versus
**3.22 ms**. Unit streams and one shared sweep favor B as the balanced production
choice without adding workload-specific dispatch.

Memory is **peak requested live heap in a separate untimed core call**, including
output and reallocations. It excludes input buffers, allocator overhead, stack,
Polars extraction/grouping and process RSS. On this machine, event/heap records
are 16 bytes aligned to 8, output segments are 32 bytes aligned to 16, and weighted
records with i128 quantities are 32 bytes aligned to 16. Two weighted record
streams request `64m` bytes before output versus `16m` for indices or unit
endpoint streams. The heap does not reserve one entry per input row, but its
start-order preparation and output still consume memory.

For the million-row output-dense weighted booking case, peak requested heap was
**131 MB** for B, **99.1 MB** for events, **83.1 MB** for indices, and **75.1 MB**
for the low-concurrency heap. On the heavily coalesced repeated-endpoint case,
B requested **64.0 MB** versus **16.0 MB** for indices. These are meaningful
costs of the contiguous weighted layout, not whole-process memory measurements.

The initial broad run used index streams as its provisional `production` label.
Its timings are historical, and are not attributed to the final implementation.
A separate i128 follow-up evaluated the adapter's actual quantity width before
the storage decision. The table above predates the later sort-guard cleanup.
See the [proof and layout notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/coverage-profile-notes.md).

</details>

## Coverage and limitations

These are synthetic fixtures on [one Windows machine](benchmarks.md#hardware),
with two seeds, two warmups and five samples per reported workload. Raw data
include tiny inputs, roughly 1k/10k/100k rows and selected million-row inputs.
They cover short intervals, variable durations, gaps, nesting, full cliques,
duplicates, repeated endpoints, net-zero boundaries, zero/empty-heavy cases,
inferred/extended/partial/outside/empty domains and several input orders.
Complete Polars cases include UInt64, Date, nanosecond zoned Datetime, null keys,
32/1,000 requested groups and differing column chunk boundaries.

Raw samples record `n`, positive clipped rows `m`, contributing distinct
coordinates plus domain edges `u`, canonical segments `z`, and peak contributing
row count `omega`. They also distinguish requested groups/chunks from observed
counts. Coordinate spans can be huge; no algorithm allocates by that span.

Small Rust/Python instances compare entire outputs with independent original-row
membership and per-integer-tick oracles. Core and candidate properties also test
transformations, partition addition, idempotence and structural invariants.
Million-row fixtures use whole-output candidate agreement and structural checks,
not a quadratic oracle. Area and peak/clique/lane comparisons are secondary
checks. All rows are validated even when the represented domain is empty.

Python timing includes argument checks, validation, extraction, partitioning,
gathering, clipping, sorting/query planning, coalescing, output materialization,
key repetition and the Python/native crossing. Rust timing includes its complete
core call and output destruction. Python output destruction, fixture construction,
verification, compilation and allocation measurements are outside timing.
Core timings cannot be substituted for complete Polars call times.

There is no universal-fastest claim. End-to-end requested heap/RSS, other machines,
other Python versions and real genomic/resource traces were not measured. The
current lazy comparison covers 8/1,000/100,000 rows, four cases and one Polars
thread, plus two grouped million-row cases. It does not measure file I/O or
24-thread lazy calls. The
native competitor covers the valid public contract on the measured fixtures;
its invalid-input error text/classes can differ. No optional map/compression
candidate, parallel sweep or new production dependency was added.

## Reproduce and data

<details markdown="1">
<summary>Reproduce this operation</summary>

Follow the [release-build setup](benchmarking.md#setup), then preserve that build
with `uv run --no-sync`. The end-to-end runner rejects stale or different native
binaries. Use new output names; neither runner overwrites saved evidence.

```sh
uv run --no-sync python benchmarks/coverage_profile_core.py --output benchmarks/results/coverage-profile-core-new.csv --weight-dtypes i128 --dtypes i64
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-polars-new
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-million-new --sizes 1000000 --cases 0 1 3 6 8 9 10 15 16
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-threads24-new --sizes 1000000 --cases 1 3 9 10 15 16
uv run --no-sync python benchmarks/coverage_profile.py --lazy --output benchmarks/results/coverage-profile-lazy-new --sizes 8 1000 100000 --cases 0 3 15 16
uv run --no-sync python benchmarks/coverage_profile.py --lazy --output benchmarks/results/coverage-profile-lazy-million-new --sizes 1000000 --cases 15 16
```

Set `POLARS_MAX_THREADS=1` before starting the one-thread end-to-end runs, and
`24` only for the thread-comparison command. Exact case filters, seeds, scopes,
commands, tool versions, source hashes and archive
names are in each run's metadata. Source ZIPs overlay the recorded base revision
and preserve uncommitted comparison implementations. The first broad run's
test file gained additional properties while timing ran; its algorithm and
runner sources were unchanged, and its original test file is archived.

Higher-case properties and reporting checks:

```sh
cargo test -p intervals-core --test coverage_profile --test coverage_profile_candidates --locked
uv run --locked --no-sync python -m pytest benchmarks/test_coverage_profile.py
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

Set `PROPTEST_CASES=2048` for the higher-case run. New minimized failures use the
existing Proptest regression convention; none were discovered in this run.

</details>

Raw comparisons:
[initial core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-20260929.csv),
[i128 follow-up](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-i128-20260929.csv),
[final core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-final-20260929.csv),
[complete Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-20260929.csv),
[million-row Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-million-20260929.csv),
[24-thread Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-threads24-20260929.csv).

[Initial eager/lazy samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-lazy-20260929.csv)
preserve the original baseline. The [current comparison](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.csv)
and [current grouped million-row samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-million-20260929.csv)
each have separate metadata and source archives for the simplified baseline.

[Verification inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-verification-20260929.md)
records the checks, release-binary identity, property case counts and packaging
results of the initial eager implementation. The
[lazy follow-up inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-lazy-verification-20260929.md)
records deferred-execution tests, current wheel checks and the focused timing run.
