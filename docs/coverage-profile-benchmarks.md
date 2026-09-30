# Coverage depth and resource demand benchmarks { #coverage-and-load-profile-benchmarks }

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`coverage_profile`](api.md#polars_intervals.coverage_profile) returns new
segments showing how many intervals are active, or their total resource demand,
along the coordinate range. With Polars restricted to one thread, complete calls
processed **100,000 synthetic intervals in 1.24–8.70 ms**, **3.9–33× faster than
the fastest tested native Polars expressions**. The advantage varies with the
data and is smaller when native Polars can use more CPU cores.

## Results

**Full Polars query time · milliseconds**

Native Polars uses grouped endpoint sums or counts to build the same profile.

<!-- Keep exact table downloads in Benchmark details below. -->
--8<-- "docs/assets/benchmarks/coverage-profile-headline.md:3:-3"

Sorted short intervals give the largest advantage in these examples. The
32-resource workload has a smaller advantage, at about four times faster.

Measurements used a September 29, 2026 build labeled 0.2.0, before sorting and
input checks changed. The updated full Polars queries have not been timed.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The package function and two equivalent native Polars queries build complete,
canonical profiles. Native queries validate input, clip endpoints, aggregate
signed events or arrival/departure counts, sort and cumulatively sum loads.
Weighted cases sum demand; other cases count active intervals. Event sums won
the displayed unweighted cases. Speedup is the fastest native median divided
by the package median for the same workload and size.

The headline selects short intervals, weighted durations and grouped cases from
the eager/lazy follow-up with its simplified native baseline. Eight-row calls
took **0.05–0.10 ms** eager, **0.14–0.26 ms** lazy and **0.23–0.46 ms** with the
streaming engine. Small apparent lazy wins are timing noise. Its
[separate million-row run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-million-20260929.metadata.json)
measured 32 clipped resources at **90.8 / 90.9 / 91.9 ms** eager/lazy/streaming,
versus **302 ms** native. For 1,000 timestamp groups, the corresponding times
were **77.6 / 78.6 / 78.8 ms**, versus **1,020 ms** native. Both seeds retained
the package advantage.

Earlier million-row eager measurements used a different native baseline.
Short intervals took **12.3 ms** ordered and **45.5 ms** shuffled, both producing
15 segments. Nested intervals produced **1,999,999 segments** in **53.7 ms**.
For repeated dates, native counts took **90.1 ms**, native event sums **118 ms**
and the package **26.1 ms**. These are seed-7 medians from that separate run.

A separate **24-thread** eager run narrowed the advantage: shuffled short
intervals took **50.9 ms** package versus **237 ms** native; 32 clipped resource
groups took **95.6 ms** versus **120 ms**. Both seeds retained the package
advantage in the six measured cases. The core sweep is sequential; other
Polars work can use the thread pool.

**Settings**

Inputs were already in memory on the shared [Windows machine](benchmarks.md#hardware).
Runs used two seeds, two warmups and five samples per workload. Displayed
medians use seed 7. Headline and other eager/lazy comparisons restricted Polars
to one thread; the thread repeat forced 24. Each run's exact scopes and commands
are recorded separately:

- [Headline eager/lazy settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.metadata.json)
- [Earlier eager settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-20260929.metadata.json)
- [Earlier million-row settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-million-20260929.metadata.json)
- [24-thread settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-threads24-20260929.metadata.json)

Python timing includes argument checks, validation, extraction, partitioning,
clipping, sorting or query planning, coalescing, materialization, group-key
assembly and the Python/native crossing. Lazy timings include schema resolution,
plan construction and collection. Rust timings include the complete core call
and output destruction. Fixture construction, compilation, correctness checks,
allocation probes and Python output destruction are outside timing. Rust times
do not represent full Polars calls.

<a id="coverage-and-limitations"></a>

**Limitations**

The synthetic matrix covers tiny to million-row inputs, input order, short and
variable durations, gaps, nesting, cliques, repeated endpoints, net-zero
boundaries and empty/zero-heavy rows. Domains include inferred, extended,
partial, outside and empty ranges. Complete Polars cases cover UInt64, Date,
zoned nanosecond Datetime, null keys, 32/1,000 requested groups and differing
chunk boundaries.

Small instances use independent original-row membership and per-integer-tick
oracles. Larger cases use whole-output candidate agreement and structural
checks; they do not have an independent quadratic oracle. Properties also
check transformations, partition addition and idempotence. Area, peak, clique
and lane comparisons are secondary checks. Every row is validated even for an
empty domain.

Output size matters: the million-row nested fixture returns almost two million
segments, while short intervals return 15. Raw samples record positive clipped
rows, distinct contributing coordinates plus domain edges, output segments,
peak active rows and observed group/chunk counts. Coordinate span does not
determine allocation.

Streaming still constructs the complete profile or group in memory. Lazy
comparisons cover 8/1,000/100,000 rows in four cases and two grouped million-row
cases. They omit file I/O and 24-thread lazy calls. End-to-end requested heap/RSS,
other machines, other Python versions and real genomic/resource traces were
not measured. Native invalid-input error classes/messages can differ. No
universal-fastest claim follows from these fixtures.

<a id="reproduce-and-data"></a>

**Reproduce**

Follow the [release-build setup](benchmarking.md#setup), then preserve that build
with `uv run --no-sync`. The runner checks the installed binary against the
release build. Use a new output name for every run.

```sh
uv run --no-sync python benchmarks/coverage_profile_core.py --output benchmarks/results/coverage-profile-core-new.csv --weight-dtypes i128 --dtypes i64
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-polars-new
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-million-new --sizes 1000000 --cases 0 1 3 6 8 9 10 15 16
uv run --no-sync python benchmarks/coverage_profile.py --output benchmarks/results/coverage-profile-threads24-new --sizes 1000000 --cases 1 3 9 10 15 16
uv run --no-sync python benchmarks/coverage_profile.py --lazy --output benchmarks/results/coverage-profile-lazy-new --sizes 8 1000 100000 --cases 0 3 15 16
uv run --no-sync python benchmarks/coverage_profile.py --lazy --output benchmarks/results/coverage-profile-lazy-million-new --sizes 1000000 --cases 15 16
```

Set `POLARS_MAX_THREADS=1` before starting the one-thread end-to-end runs, and
`24` only for the thread-comparison command. Each run's metadata records exact
filters, seeds, scopes, commands, versions and source hashes. Source ZIPs overlay
the recorded base revision and preserve uncommitted comparison implementations.

Higher-case properties and reporting checks:

```sh
cargo test -p intervals-core --test coverage_profile --test coverage_profile_candidates --locked
uv run --locked --no-sync python -m pytest benchmarks/test_coverage_profile.py
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

Set `PROPTEST_CASES=2048` for the higher-case run.

Exact table downloads, including supplemental eager, million-row, thread and
algorithm comparisons:

Headline cases:

--8<-- "docs/assets/benchmarks/coverage-profile-headline.md:-2:"

Eager, lazy and streaming comparisons:

--8<-- "docs/assets/benchmarks/coverage-profile-lazy-table.md:-2:"

Earlier eager comparisons:

--8<-- "docs/assets/benchmarks/coverage-profile-polars-table.md:-2:"

Earlier million-row comparisons:

--8<-- "docs/assets/benchmarks/coverage-profile-million-table.md:-2:"

24-thread comparisons:

--8<-- "docs/assets/benchmarks/coverage-profile-threads24-table.md:-2:"

Private Rust algorithm comparisons:

--8<-- "docs/assets/benchmarks/coverage-profile-core-table.md:-2:"

Raw comparisons:
[initial core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-20260929.csv),
[i128 follow-up](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-i128-20260929.csv),
[final core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-final-20260929.csv),
[complete Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-20260929.csv),
[million-row Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-million-20260929.csv),
[24-thread Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-threads24-20260929.csv).

[Initial eager/lazy samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-lazy-20260929.csv)
preserve the original baseline. The [simplified-baseline comparison](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.csv)
and [grouped million-row samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-million-20260929.csv)
each have separate metadata and source archives for the simplified baseline.

[Verification inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-verification-20260929.md)
records the checks, release-binary identity, property case counts and packaging
results of the initial eager implementation. The
[lazy follow-up inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-lazy-verification-20260929.md)
records deferred-execution tests, current wheel checks and the focused timing run.

**History**

The headline run was recorded at `2026-09-29T15:59:30.655100+00:00`, based on
`2b2a2666386c971f6e8d63cbf47cd7a405d1f41b` with uncommitted feature sources.
Its [source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.sources.zip)
declares version `0.2.0`; core and adapter match `280e515`. The base revision
alone does not identify the measured code. Later `8284b66` sorting changes and
shared input-check/conversion helpers affect timed paths. Runner provenance
bookkeeping is outside timing. The
[before/after sort review](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-sort-review-20260929.md)
found matching outputs and allocations but variable timing differences. It
does not provide new full Polars timings.

The broader eager runs predate the lazy wrapper and native baseline's
join simplification. The initial lazy run also retains its earlier baseline.
Each source snapshot remains separate; no saved timing is attributed to a
changed wrapper or baseline. The Rust engine was unchanged in the lazy
follow-up. The first broad run's test file gained properties during timing;
algorithm and runner sources were unchanged, and the original test file is
archived.

The [private algorithm/layout notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/coverage-profile-notes.md)
explain the retained two-stream engine and the event, heap and weighted-index
candidates. Complete sequential Rust calls used five samples with
[separate core settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-final-20260929.metadata.json).
At one million shuffled weighted bookings, stream records took **131 ms**,
events **135 ms**, heap **213 ms** and indices **254 ms**. The small event
difference does not establish a reliable winner. Ordered weighted bookings
favored the heap: **69.4 ms** versus streams **94.1 ms** and indices **75.0 ms**.
At 100,000 repeated endpoints, indices took **2.59 ms** versus streams **3.22 ms**.
The earlier broad run's `production` label meant provisional index streams;
the i128 follow-up and final core results preserve that distinction.

Core memory is **peak requested live heap in a separate untimed call**,
including output and reallocations. It excludes inputs, allocator overhead,
stack, Polars extraction/grouping and process RSS. Two i128 weighted record
streams request `64m` bytes before output for `m` positive clipped rows, versus
`16m` for index or unit endpoint streams. Million-row output-dense weighted bookings requested
**131 MB** for streams, **99.1 MB** events, **83.1 MB** indices and **75.1 MB**
heap. Repeated endpoints requested **64.0 MB** for streams versus **16.0 MB**
indices. These are costs of the faster shuffled-input layout.

</details>
