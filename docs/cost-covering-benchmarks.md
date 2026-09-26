# Minimum-cost covering benchmarks

[All benchmarks](benchmarks.md) · [Running and publishing](benchmarking.md)

## Summary

`minimum_cost_cover` selects a continuous cover with minimum total cost, breaking cost
ties by the fewest intervals. Production uses a reversed Fenwick suffix-minimum dynamic
program: `O(n log n)` time and `O(n)` additional space. It uses less storage than the
segment-tree reference and wins many workloads with many reachable frontiers, with some
measured exceptions.

The unweighted operation has its own [minimum-covering report](covering-benchmarks.md).

## Compared implementations

| Candidate | Implementation | Role |
| --- | --- | --- |
| MCC-A | Reversed Fenwick suffix-min frontier DP | Production and instrumented benchmark |
| MCC-B | Iterative segment-tree range-min frontier DP | Tests/benchmarks |
| MCC-C | Direct scan of prior reachable frontier states | Tests/benchmarks, at most 1K rows |

Both trees stop ancestor updates when the stored minimum does not improve. The
segment-tree reference retains no redundant second DP array. Production has no method
switch or graph/solver dependency.

### Semantics and exactness

All intervals and the single target are half-open. Touching intervals chain. Empty
targets return all false after validation. Empty intervals never help. Outside rows are
discarded after validation, and useful rows are clipped to the target. Reversed
targets/intervals and null endpoints are errors. A non-empty target without a continuous
cover raises `target interval cannot be covered by the supplied intervals`.

Sort candidates by effective right end, compress right ends plus target start, and
maintain the best `(cost, count, frontier_index)` at reachable frontiers. A reversed
Fenwick tree queries the minimum over reachable frontiers `x >= l`. Only frontiers
strictly below `r` have been published when processing `[l,r)`. Add the interval's cost
and one selected interval. Publish the best state only after all candidates ending at
`r` have been queried. Backpointers reconstruct the original row mask.

This recurrence is exact: any advancing interval extends a reachable continuous prefix,
and any nonredundant cover can be ordered by its advancing right endpoints. Nonnegative
costs and the secondary count objective ensure that a nonadvancing interval is never
necessary. The cheapest local interval is not a valid greedy rule: `[0,4):1`, `[0,6):5`,
`[4,10):100`, `[6,10):5` has optimum 10, although starting with the cheapest interval
can cost 101.

Weighted comparisons first minimize cost, then count. Remaining ties use row index and
predecessor coordinate. Deterministic masks are guaranteed for identical input, but a
particular tied mask is not a stable public contract. Costs must be nonnegative
integers. Python/Polars accepts the eight integer dtypes through 64 bits, without nulls
or casts. The core accepts integer types convertible to `i128`. Addition is checked.
Overflowing paths cannot return to a representable cost because costs are nonnegative.
Such paths are discarded. If the final state is absent after an overflow, an unweighted
feasibility check distinguishes infeasible coverage from an optimum exceeding
`i128::MAX`.

The production algorithm takes **O(n log n) time and O(n) additional space**, including
output. Reconstruction is linear after sorting and dynamic programming. For `m` useful
rows and `q <= m+1` distinct frontier coordinates, weighted storage is `O(m+q+n)`:
candidates, coordinates, one Fenwick tree, backpointers and the output mask. The tree is
released before allocating the output mask. The Polars adapter borrows contiguous
physical endpoint slices. Multiple chunks require copying. Costs are widened once to
`i128`.

## Results

Measured on Windows 11 x86-64, AMD Ryzen 9 3900X (12 cores / 24 threads), Rust 1.98.1,
Python 3.14 and Polars 1.44.2, with the optimized Cargo bench profile. The shared
covering run has 7,425 timed samples across 396 workloads and 2,475 candidate/workload
combinations, with three repeats each. These counts cover both operations, not just the
operation on this page.

--8<-- "docs/assets/benchmarks/cost-cover-runtime.md"

### Wider workload comparison

Median per-workload runtime ratios (candidate / Fenwick baseline). Values below one favor the candidate.
Each geometry/order/cost/size combination has equal weight:

| Candidate | Sorted | Nearly sorted | Shuffled |
| --- | ---: | ---: | ---: |
| MCC-B / MCC-A | 1.237 | 1.209 | 1.122 |
| MCC-C / MCC-A, 1K only | 1.254 | 1.250 | 1.141 |

Sorted/shuffled include physical-width runs. Nearly sorted does not. Small compressed
state spaces hide the quadratic reference's poor scaling: at 1K rows its medians are
1.51 ms on dense data and 2.25 ms on equal starts, versus 0.089 ms for Fenwick on the
shuffled dense/random case.

Selected 1M-row shuffled medians, milliseconds:

| Workload | Fenwick | Segment tree |
| --- | ---: | ---: |
| Touching chain | 188.08 | 205.33 |
| Dense overlaps | 226.50 | 389.63 |
| Equal starts | 198.61 | 274.19 |
| Mostly irrelevant | 2.05 | 2.04 |
| Date-width chain | 210.87 | 195.95 |
| Date-width dense | 275.19 | 448.58 |

The Date-width chain favors the segment tree. Fenwick is not a universal winner. For the
1M-row i64 chain, Fenwick/segment-tree peak requested allocations are 105.17/157.83 MB.
For dense input they are 87.17/151.83 MB. Irrelevant-row workloads need about 1 MB,
mostly output. The maximum shared-run peak is 157.83 decimal MB, not RSS.

For the shuffled chain, Fenwick preprocessing/DP/reconstruction medians are
50.26/131.92/3.60 ms, with 23 allocation/reallocation events. Dense phases are
50.28/172.78/0.063 ms. Raw samples retain phase and memory data, including failures.

### End-to-end temporal measurements

The release wheel installed outside the checkout produced 1,536 samples across 512
dtype/workload combinations for both covering operations. These selected 1M-row
shuffled-chain medians include plugin, validation, adapter and output overhead. The
native fixtures and costs differ from the core harness. Subtracting their times does not
estimate adapter overhead.

| Endpoint dtype | minimum_cost_cover (ms) |
| --- | ---: |
| Int32 | 196.09 |
| Int64 | 210.80 |
| UInt64 | 208.29 |
| Date | 196.34 |
| Datetime(ms) | 213.34 |
| Datetime(us) | 206.46 |
| Datetime(ns, UTC) | 221.16 |
| Datetime(ns, Europe/Helsinki) | 205.19 |

## Workloads and correctness

--8<-- "benchmarks/covering-methodology.md"

## Reproduce

| File | Role |
| --- | --- |
| [`covering.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/covering.rs) | Run both covering candidate suites |
| [`covering_summary.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering_summary.py) | Summarize a supplied candidate CSV |
| [`covering_temporal.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering_temporal.py) | Time both operations with integer/temporal release-plugin inputs |
| [`plot.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plot.py) | Generate separate figures for each operation |

Build the release plugin using the shared setup guide before running the temporal suite.
Save new runs separately from the published measurements:

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_summary.py benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_temporal.py > benchmarks/results/covering-temporal-local.csv
```

`COVER_BENCH_MAX` and `COVER_BENCH_SAMPLES` limit candidate size and repeats. Defaults
are one million rows and three samples. The harness measures both operations. The chart
configuration selects the relevant method IDs.

## Limitations

Raw CSV columns retain every sample's total, preprocessing (validation, clipping,
sorting/compression), optimization (sweep/DP), and reconstruction time. Greedy selection
writes the mask during its sweep. Its reconstruction time is zero. The weighted mask is
reconstructed in a separate measured phase. Total time also includes temporary-buffer
destruction and phase-clock overhead, so it need not equal the sum of phase times. Early
failures legitimately skip later phases.

Peak live requested allocation bytes and allocation/reallocation event counts are
measured in a **separate untimed invocation** using the system allocator. These include
algorithm buffers and output, but exclude caller-owned inputs, oracle work, allocator
metadata, stacks and process RSS. Measurements are serial. Neither solver introduces
internal workers.

This is synthetic evidence from one Windows x86-64 machine, not a cross-platform speed
guarantee. Short timings are sensitive to scheduler and clock noise. Use per-workload
medians, not individual minima, to compare methods.

There is no ordinary native Polars expression baseline: choosing a global subset
requires an iterative frontier algorithm or dynamic programming with reconstruction. An
overlap count, group aggregate, join, or sorted union is not an equivalent minimum
cover. No misleading substitute is timed.

## Raw data

- [Shared candidate samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv)
- [Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log)
- [Environment and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)
- [Installed release-wheel samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv)

The original source hashes remain attached to the measurements. Cleanup verification
records the consolidated harness/adapter sources separately. The measured solver and
candidate implementations were unchanged.

### Historical validation

The following counts belong to the original covering implementation and measurement
revision.

The completed local validation includes 99 Rust tests/doctests, 1,229 Python
tests/doctests (also all passing against the installed external release wheel), and 49
CI/release helper tests. Formatting, Clippy across all targets with denied warnings,
Rustdoc with denied warnings, the uv lock check, Ruff lint/format and strict MkDocs
builds pass. A release wheel and source archive were built and audited for metadata,
license, type marker, native extension and new source files. The full 15-wheel
cross-platform CI matrix is not a local test and was not run.
