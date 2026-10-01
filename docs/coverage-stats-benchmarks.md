# Per-query coverage statistics benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

The [`coverage_stats`](api.md#polars_intervals.coverage_stats) function counts
source records and measures covered length separately for every query. It
preserves query rows and payloads. In these synthetic workloads, complete lazy
medians were **at least 8.1× faster than Polars and under 480 ms for one million
queries against one million source rows**. Many tiny timestamp groups narrow
the advantage. Wide payloads also increase the native plan's time and memory.

## Results

**Full Polars query time · milliseconds**

Native Polars: the tested query using grouped endpoint events and cumulative
counts. Both functions return all four statistics and preserve query payloads.
The table includes planning and complete lazy collection. Here **n** is query
rows and **m** is source rows. The two sizes are equal in this table.

--8<-- "docs/assets/benchmarks/coverage-stats-summary.md:3:-3"

Counts retain source multiplicity. Covered length counts each coordinate once,
even in the one-run case where every source overlaps every query. Small grouped
inputs have a smaller speed advantage than these large examples.

Strongly unequal inputs remain separate: the first row has **n = 8, m = 1M**.
The second has **n = 1M, m = 8**.

--8<-- "docs/assets/benchmarks/coverage-stats-ratios.md:3:-3"

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The public function and native plan both validate both collections. The native
plan aggregates duplicate source endpoints, sorts tagged source and query
boundaries, and accumulates record counts. It integrates Boolean occupancy,
not coverage depth. A Struct carries query payloads on query-start requests.
A unique ordinal restores every query. No overlap pairs, Python row/group
loops or nested lazy collections are used. Shared native validation is included
in timing. This is the only native competitor, so it supplies every native cell.

--8<-- "docs/assets/benchmarks/coverage-stats-summary.md:-2:"
--8<-- "docs/assets/benchmarks/coverage-stats-ratios.md:-2:"

The broad run includes 22 input patterns. These matched cases isolate narrow
inputs, wide query payloads and irrelevant source payloads. The wide query has
16 string columns plus list and struct columns. Source payloads are projected
away. The source-only wide control was slower in this run, so its cost is not
reported as zero.

--8<-- "docs/assets/benchmarks/coverage-stats-payloads.md:3:-3"
--8<-- "docs/assets/benchmarks/coverage-stats-payloads.md:-2:"

The separate mode run distinguishes planning from execution. Collecting a
prebuilt plan still includes validation, source preparation and all statistics.
Mixed and streaming rows include planning. Parquet rows include warm local reads.

--8<-- "docs/assets/benchmarks/coverage-stats-modes.md:3:-3"
--8<-- "docs/assets/benchmarks/coverage-stats-modes.md:-2:"

**Underlying algorithm time · milliseconds**

Production uses direct cursors when both query bounds are actually sorted and
`n >= m`. Otherwise it uses packed boundary sweeps when `n <= m` and the source
union has at least 32 runs. It uses preallocated binary searches otherwise.
The empirical cutoff preserves the worst-case bound
`O(m log(m+1) + n log(m+1) + n)` with `O(m+n)` additional storage.

--8<-- "docs/assets/benchmarks/coverage-stats-core.md:3:-3"

The conservative rule retains a real loss: the shuffled genomic core case took
400 ms in production versus 271 ms for packed sweeps. Sweeps also lose on the
one-run and query-heavy cases. See the
[proofs and selection evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/coverage-stats-notes.md)
for repeated crossover measurements and the exact rule.

--8<-- "docs/assets/benchmarks/coverage-stats-core.md:-2:"

**Settings**

Polars used its default **24-thread** pool with `POLARS_MAX_THREADS` unset.
The environment was Python 3.14.0 and Polars 1.44.2. The broad run has two
warmups, five samples and seeds 7/41. The million run has one warmup, three
samples and seeds 7/41. The mode run has one warmup, three samples and seed 7.
Tables display seed 7, except the labeled genomic repeat using seed 41.
Repeat seeds and different runs are never pooled.
The final core run has two warmups, five samples and seeds 7/41.

Full calls include both validations, grouping, preparation, all statistics,
payload attachment and materialization. Fixture creation, correctness and
returned-DataFrame destruction are excluded. Core clocks include returned-output
destruction. Candidates rotate per sample. See shared
[hardware](benchmarks.md#hardware) and [measurement rules](benchmarking.md#measurement-rules).

Parquet query/source row-group sizes are `max(1, n//7)` and `max(1, m//11)`.
Raw chunk fields describe the initial in-memory fixtures. Actual collection
batches depend on the engine.

Small fixtures use independent original-row cell and bounded-tick oracles.
Extreme Python checks use arbitrary-precision subtraction and a `1e-15` fraction
tolerance. Every timed output is checked outside timing, including dtypes,
nullness and row order. Large comparisons are candidate agreement. They do not
independently validate the shared mathematical reduction.

**Limitations**

These are synthetic inputs on one machine. Only selected narrow complete-lazy
cases reach one million rows per operand. Eager, mixed, streaming, scans, wide
payloads and process-memory observations stop at 100,000 rows per operand.
Cold scan I/O and out-of-core execution are not measured. Both routes may block
under the streaming engine. No native median won in the measured full-call
matrix, but this does not compare every possible native plan or hardware setup.

Separate cold eager processes measured **peak RSS increase after fixture
construction**. Matched narrow inputs at `n=m=100k` increased the peak by
15.7 MB for the package and 695 MB for native Polars. Wide queries and sources
increased it by 15.8 MB and 3.31 GB. Both RSS readings cover the whole process,
including inputs, runtime and retained output. Their difference is not an
allocation count or algorithm heap.

Separate core allocator calls measured **peak requested live heap**, including
preparation and output. Final production examples at `n=m=1M` used 80.0–146 MB.
That metric excludes caller inputs, allocator overhead, stack and RSS. These
memory measurements cannot be substituted for one another.

**Reproduce**

Follow the shared [release setup](benchmarking.md#setup), then run:

```sh
uv run --no-sync pytest benchmarks/test_coverage_stats.py
uv run --no-sync python -m benchmarks.coverage_stats --output benchmarks/results/coverage-stats-polars-new
uv run --no-sync python -m benchmarks.coverage_stats --output benchmarks/results/coverage-stats-polars-million-new --sizes 1000 1000000 --cases 1 3 7 8 10 14 16 17 18 --scopes lazy_complete --samples 3 --warmups 1
uv run --no-sync python -m benchmarks.coverage_stats --output benchmarks/results/coverage-stats-polars-modes-new --sizes 1000 100000 --cases 1 11 12 19 20 21 --scopes lazy_plan lazy_collect mixed_complete streaming_complete --seeds 7 --samples 3 --warmups 1 --memory
```

The runner verifies that the installed plugin matches the fresh release library.
Use new output prefixes. Raw samples retain separate `n`/`m`, union-run counts,
multiplicity, distinct boundaries, query spans, groups, order, dtype, payload,
chunks, source, seed and scope. Saved evidence:

- Broad run: [3,440 samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-local.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-local.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-local.sources.zip).
- Million run: [216 samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-million.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-million.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-million.sources.zip).
- Mode run: [276 samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-modes.csv), [20 RSS observations](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-modes.memory.json), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-modes.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-modes.sources.zip).
- Final core: [630 samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-core-production.csv), [metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-core-production.metadata.json), [sources](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-core-production.sources.zip).

**History**

The measured checkout was based on `cf02aabb` with the archived feature changes.
All three full-call runs used the same verified release library, and their
measured sources stayed unchanged during each run. The final core run measures
the actual selected route. Earlier binary, sweep and provisional dispatch
experiments remain separate in the selection notes, with their own raw samples,
metadata and source archives. Their timings are not relabeled as production.

</details>
