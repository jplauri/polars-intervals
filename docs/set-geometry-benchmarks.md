# Subtraction and intersection benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`subtract_intervals`](api.md#polars_intervals.subtract_intervals) removes
right-side coverage from the left collection.
[`intersect_intervals`](api.md#polars_intervals.intersect_intervals) returns
coverage shared by both collections. Both return maximal nonempty ranges.
In synthetic benchmarks, complete lazy calls took **under 41 ms for one million
total input rows**. The package was slower than Polars on empty-only grouped
inputs at that size.

## Results

**Full Polars query time · milliseconds**

Native Polars uses grouped endpoint events, separate coverage counts for each
source and coalescing. Times include planning and complete lazy collection.
Input sizes count both collections together.

**Subtraction**

--8<-- "docs/assets/benchmarks/subtract-intervals-summary.md:3:-3"

**Intersection**

--8<-- "docs/assets/benchmarks/intersect-intervals-summary.md:3:-3"

The advantage depends on geometry. The native event plan avoids most geometry
work when all intervals are empty. The package still validates and groups every
row. In this separate repeat, it leads at 100,000 empty rows and trails at one
million:

--8<-- "docs/assets/benchmarks/set-geometry-empty-summary.md:3:-3"

Across the million-row matrix, repeated Date intervals had the smallest
advantage, **4.0×**. Sorted availability was faster than shuffled input for the
package.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The native Polars comparison uses tagged endpoint events, signed counts per
source, grouped cumulative sums and gap-aware coalescing. It includes strict
schema checks, both-side row validation, null-key matching and left-first group
order. Its blocking validation callback performs native Polars operations.
The geometry plan has no Python row or group loops and no nested collection.
It is the only native competitor, and therefore the native method in every
table row. The main tables use the five-sample repeat with seed 7. The other
seed preserves the same direction of comparison. Empty-only inputs use a
separate repeat and are not pooled with the main run.

--8<-- "docs/assets/benchmarks/subtract-intervals-summary.md:-2:"
--8<-- "docs/assets/benchmarks/intersect-intervals-summary.md:-2:"
--8<-- "docs/assets/benchmarks/set-geometry-empty-summary.md:-2:"

Eager million-row shuffled availability took **48.6 / 481 ms** for
subtraction and **37.9 / 320 ms** for intersection, package/native. At 100,000
sorted rows, package planning took **0.102–0.108 ms**, versus
**0.449–0.468 ms** native. Collecting prebuilt plans took **2.00–2.80 ms**,
versus **32.1–38.1 ms**.

Warm Parquet queries on 100,000 synthetic genomic-style rows took
**8.89–9.24 ms** package versus **38.1–38.8 ms** native, including reads but
excluding prebuilt-plan construction. Complete streaming-engine calls took
**10.0–10.3 ms**, versus **43.7–45.5 ms**. The raw phase run also retains mixed
eager/lazy calls and grouped timestamp cases.

The private Rust comparison measures canonical unions followed by scans (A),
on-demand merged runs over packed pairs (B), and tagged endpoint events (C).
The package retains A for both functions. It reuses merge preparation and has
better behavior on dense, nested and touching inputs. B improves some shuffled
intersections but retains both packed input buffers. It does not provide a
consistent runtime or memory win.

**Underlying algorithm time · milliseconds**

--8<-- "docs/assets/benchmarks/set-geometry-core.md:3:-3"

At 100,000 dense rows, peak requested heap was **0.800 MB** for A and
**2.10 MB** for B. With output-linear availability, both used **3.15 MB**
for intersection and **4.19 MB** for subtraction. These separate allocator
measurements include preparation and output. They exclude caller inputs,
allocator overhead, stack and RSS. The
[experiment notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/set-geometry-notes.md)
explain the candidates and retain their useful wins and losses.

--8<-- "docs/assets/benchmarks/set-geometry-core.md:-2:"

**Settings**

See shared [hardware](benchmarks.md#hardware) and the
[measurement guide](benchmarking.md). Sizes count both operands together.
Polars used its default **24-thread** pool, with `POLARS_MAX_THREADS` unset.
The environment was Python 3.14.0 and Polars 1.44.2. Broad runs use one warmup
and three samples. Headline, million-row, phase and empty-row repeats use two
warmups and five samples. Main repeats use seeds 7 and 41, kept separate.
Fixtures and correctness checks are outside timing. Candidate order rotates
between samples. Rust timings include returned-output destruction. Polars
timings exclude it. Separate scopes distinguish eager calls, complete lazy
calls, plan construction, prebuilt-plan collection, mixed calls and streaming
engine collection.

Small cases compare complete outputs with an independent original-row cell
oracle. Rust also uses a bounded bitmap oracle. Larger samples check complete
candidate agreement and canonical output invariants. Candidate agreement is
not an independent check of the original problem.

**Limitations**

These are synthetic workloads on one Windows machine. No out-of-core behavior
or universal speed advantage is claimed. Both lazy inputs can materialize at
the blocking node. Narrow Int16 core fixtures are omitted when their
coordinates exceed the dtype. Scan-backed cases measure warm Parquet reads
separately from in-memory calls. There is no all-pairs join comparison.

Only selected layouts reach one million rows. Wide payloads, sparse key matches
and Parquet inputs were measured through 100,000 rows. Benchmarks do not cover
every endpoint/key dtype, cold storage, thread count or ratio of input sizes.
The public implementation is retained for its broad results and shared merge
code. Its empty-only grouped-input loss remains a limitation.

Separate fresh-process eager measurements recorded peak RSS increases after
fixture creation. Million-row shuffled availability added **42.1 / 565 MB**
for subtraction and **33.6 / 511 MB** for intersection, package/native.
This is an operating-system high-water mark, including runtime and retained
output. It is not requested heap or allocation count. Earlier peaks can mask
query memory. Each observation is one cold process, not a distribution.

**Reproduce**

Follow the [release-build setup](benchmarking.md#setup). Preserve the installed
release plugin with `--no-sync`:

```sh
uv run --no-sync python -m benchmarks.set_geometry_core --output benchmarks/results/set-geometry-core-new
uv run --no-sync python -m benchmarks.set_geometry --output benchmarks/results/set-geometry-polars-new
uv run --no-sync pytest benchmarks/test_set_geometry.py
```

Reproduce the headline selection with `--cases 1 3 10 --sizes 1000 100000
1000000 --scopes lazy_complete --seeds 7 41 --samples 5 --warmups 2`. The
empty-only repeat uses `--cases 16` and `--scopes eager lazy_complete`.

Both runners refuse to overwrite saved evidence and preserve source ZIPs,
settings, seeds, versions and hashes. The Polars runner verifies that the
installed plugin matches the fresh release library. Use `--cases`, `--sizes`,
`--seeds`, `--samples` and `--warmups` for targeted repeats. Its `--scopes`
option separates timing boundaries and `--memory` records separate cold
process RSS observations.

Complete Polars evidence:

- Broad: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20261001.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20261001.sources.zip).
- Headline repeat: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20261001.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20261001.sources.zip).
- Million-row matrix: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20261001.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20261001.sources.zip).
- Empty-row repeat: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20261001.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20261001.sources.zip).
- Planning, collection and scans: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20261001.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20261001.sources.zip).
- Eager and memory: [timings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20261001.csv), [RSS observations](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20261001.memory.json), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20261001.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20261001.sources.zip).

**History**

The Polars runs above measured revision `5d63538`. The core runs measured
`5fd05e0` plus the saved feature changes, with unchanged sources throughout
each run:

- Broad core: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-20260930.csv),
  [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-20260930.metadata.json),
  [source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-20260930.sources.zip).
- Repeated 100,000-row core: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-repeat-20260930.csv),
  [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-repeat-20260930.metadata.json),
  [source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-repeat-20260930.sources.zip).
- Million-row core: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-million-20260930.csv),
  [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-million-20260930.metadata.json),
  [source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-million-20260930.sources.zip).

A later cleanup removed a redundant empty-left check from `intersect_intervals`.
The core timings measure the archived source. A benchmark-only seed-overflow
fix leaves measured seeds 7 and 41 unchanged.

The first lazy adapter tagged every row with its source and position, then
reordered both operands in Rust. On `5fd05e0` plus the saved feature changes,
million-row shuffled availability took 83.8 / 81.0 ms for subtraction /
intersection in lazy calls. Million-row empty-only groups took 84.7 / 85.6 ms.
Those runs remain available:

- Broad: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20260930.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-20260930.sources.zip).
- Headline repeat: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20260930.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20260930.sources.zip).
- Million-row matrix: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20260930.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-million-20260930.sources.zip).
- Empty-row repeat: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20260930.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20260930.sources.zip).
- Planning, collection and scans: [samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20260930.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-phases-20260930.sources.zip).
- Eager and memory: [timings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20260930.csv), [RSS observations](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20260930.memory.json), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20260930.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-memory-20260930.sources.zip).

Original sources remain in the archives. No historical timings have been
relabeled as measurements of changed production code.

</details>
