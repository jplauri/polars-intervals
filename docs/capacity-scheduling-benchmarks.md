# Capacity-constrained weighted selection

`max_weight_with_capacity` selects an exact globally maximum-weight subset of
half-open intervals, with at most `capacity` selected non-empty intervals active
at any point. It exposes a Boolean mask through Python expressions, Rust Polars,
and the Polars-independent core.

## Candidates and decision

The benchmark compiles the production source directly into a private target;
no benchmark API or generic graph API is exported by either library.

| Candidate | Implementation |
| --- | --- |
| `whole` | Specialized successive shortest paths, one network, only empty/nonpositive/zero-capacity filtering |
| `whole_guarded` | The same network with a sufficient-capacity check |
| `components` | Specialized serial flow per overlap component, with sufficient-capacity checks |
| `parallel` | The same component solver, forced to up to eight scoped workers when there are at least eight components |
| `generic` | Independent conventional adjacency-list min-cost flow, Bellman-Ford initial potentials and heap-based shortest augmenting paths |
| `capacity_one` | The unchanged existing `max_weight_non_overlapping` kernel, measured only at capacity 1 |
| `production` | Selected design: all fast paths, specialized component flow, thresholded parallelism |

The production network uses compact sorted endpoint indices, paired residual
edges in one contiguous buffer, and CSR adjacency indices. An initial DAG pass
finds both the first path and feasible potentials. Later paths use Dijkstra with
exact reduced costs, reusing distance, predecessor and heap buffers. Strict
relaxation and `(distance, vertex)` heap ordering make ties deterministic.
Augmentations send the path bottleneck, including multiple idle units when
appropriate; a directly invoked network sends exactly k units.

The generic implementation stays private for reproducibility and cross-checks.
It adds no dependency. The only new direct package dependency is `serde`, already
present transitively, for the plugin's capacity option. The independent core
still has no production dependencies.

A fixed-small-k DP was considered but not implemented: storing combinations of
resource finish states would introduce an `O(n^k)` subsystem, outside the requested
complexity budget. The DAG initial path already handles k=1 efficiently inside
flow, but it does not eliminate network construction and compression overhead.
The existing capacity-1 scheduler remains unchanged.

## Measurements on this machine

Windows 11, Ryzen 9 3900X (12 cores / 24 logical processors), about 32 GiB RAM,
Rust 1.98.1 / LLVM 22.1.8, optimized Cargo bench profile. Values below are medians
of three timed samples, in milliseconds. These are observed tradeoffs, not
universal performance guarantees. All table cases use positive weights and
shuffled rows unless noted.

### Scaling with size and density

| Family | n | k | Production | Whole flow | Generic flow |
| --- | ---: | ---: | ---: | ---: | ---: |
| disjoint | 1,000 | 2 | 0.037 | 0.315 | 0.771 |
| disjoint | 10,000 | 2 | 0.415 | 2.332 | 8.316 |
| disjoint | 100,000 | 2 | 6.188 | 39.759 | 163.977 |
| disjoint | 1,000,000 | 2 | 85.645 | 443.359 | 2447.219 |
| moderate | 1,000 | 2 | 0.230 | 0.227 | 0.469 |
| moderate | 10,000 | 2 | 2.928 | 2.912 | 5.729 |
| moderate | 100,000 | 2 | 37.944 | 37.613 | 95.620 |
| moderate | 1,000,000 | 2 | 465.281 | 443.179 | 1589.626 |
| dense | 1,000 | 2 | 0.331 | 0.325 | 0.681 |
| dense | 10,000 | 2 | 4.786 | 4.762 | 11.504 |
| dense | 100,000 | 2 | 67.264 | 66.854 | 185.761 |
| dense | 1,000,000 | 2 | 1086.622 | 1059.819 | 3224.795 |
| components | 1,000 | 2 | 0.218 | 0.261 | 0.464 |
| components | 10,000 | 2 | 2.214 | 3.241 | 7.234 |
| components | 100,000 | 2 | 9.688 | 45.787 | 125.561 |
| components | 1,000,000 | 2 | 128.445 | 501.523 | 1968.722 |

### Capacity and component effects (100K rows, independent components)

| k | Production | Serial components | Forced parallel | Whole flow |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 0.098 | 0.074 | 0.076 | 0.122 |
| 1 | 4.888 | 14.634 | 8.522 | 28.048 |
| 2 | 9.688 | 24.065 | 10.137 | 45.787 |
| 4 | 12.145 | 42.020 | 12.063 | 80.458 |
| 8 | 17.769 | 76.824 | 16.951 | 152.191 |
| 15 | 22.689 | 119.031 | 22.160 | 237.657 |
| 16 | 6.439 | 6.289 | 6.653 | 262.346 |
| 17 | 6.456 | 6.167 | 6.401 | 250.405 |
| 64 | 6.355 | 6.357 | 6.430 | 251.721 |

Concurrency is 16. The runtime drop at k=16 is the sufficient-capacity bypass.
At k=8, decomposition alone cuts 152.2 ms to 76.8 ms; thresholded parallel
production takes 17.8 ms. At 1M rows and k=2, serial components take 267.8 ms
versus 501.5 ms for one network, while production takes 128.4 ms.

### Capacity one (1M rows)

| Family | Existing DP | Production delegate | Whole flow | Forced parallel components |
| --- | ---: | ---: | ---: | ---: |
| disjoint | 123.305 | 121.830 | 361.986 | 84.287 |
| moderate | 125.075 | 123.991 | 267.893 | 298.170 |
| dense | 119.478 | 127.079 | 329.167 | 368.254 |
| components | 122.181 | 121.229 | 332.898 | 106.680 |

The existing kernel is unchanged and delegation tracks its performance.
General flow is usually materially slower. One exception is the largest
component workload, where forced parallel flow beats the serial DP modestly.
This does not justify adding preprocessing to every capacity-1 call; retain
the direct specialization. The generic solver can also be competitive on
highly compressed identical-interval networks; no universal winner is claimed.

### Peak requested allocation (1M rows, k=2; MiB)

| Family | Production | Serial components | Whole flow | Generic flow |
| --- | ---: | ---: | ---: | ---: |
| disjoint | 56.6 | 56.6 | 377.0 | 583.8 |
| moderate | 255.1 | 255.1 | 255.1 | 332.0 |
| dense | 409.0 | 409.0 | 409.0 | 583.8 |
| components | 96.4 | 56.6 | 316.0 | 457.9 |

For the 1M component case, production records 437,664 allocation/reallocation
events, versus 1,500,072 for the generic engine and just 37 growing-buffer
allocations for whole specialized flow. Component solving allocates per component
to reduce the live network size. Serial decomposition has lower
peak memory than parallel decomposition; bounded parallelism spends extra row
and mask buffers to reduce wall time. See the measurement definition below.

### Weight distributions (1K rows, moderate overlap, k=8)

| Weights | Production | Whole flow | Generic flow |
| --- | ---: | ---: | ---: |
| positive | 0.921 | 0.905 | 1.140 |
| mixed | 0.555 | 0.541 | 0.659 |
| zeros | 0.156 | 0.154 | 0.199 |
| equal | 0.380 | 0.428 | 0.563 |
| ties | 0.849 | 0.840 | 1.077 |
| skewed | 0.783 | 0.784 | 1.039 |
| valuable_long | 0.514 | 0.505 | 0.665 |

Removing nonpositive rows reduces both network size and candidate concurrency.
All accumulations, including the skewed distribution, remain integer-exact.

### Release plugin with temporal endpoints (1M rows)

| k | Int64 | Date | Datetime(us) | Datetime(ns, UTC) |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 7.434 | 7.910 | 7.861 | 7.680 |
| 1 | 28.005 | 44.195 | 28.329 | 28.503 |
| 2 | 62.495 | 49.128 | 83.281 | 79.119 |
| 4 | 69.884 | 63.493 | 69.180 | 71.812 |
| 8 | 82.290 | 67.973 | 82.286 | 85.279 |
| 16 | 97.413 | 91.692 | 103.325 | 97.787 |
| 31 | 118.605 | 100.940 | 125.659 | 115.470 |
| 32 | 43.250 | 31.733 | 41.008 | 40.833 |
| 33 | 43.336 | 36.196 | 44.719 | 40.886 |
| 64 | 40.975 | 30.640 | 40.946 | 40.540 |

These are end-to-end query times, including the adapter and output creation,
on independent 32-row cliques. Casts are outside timing. They do not isolate
the cost of temporal dtype handling. All 160 dtype/size/capacity cases matched
the independent top-k oracle and capacity check; 480 timed samples are retained.

The native run contains **2,014 workloads and 36,984 timed samples**. It covers
14 empty cases, 1,388 at 1K, 362 at 10K, 158 at 100K, and 92 at 1M. The work
budget explicitly omits 182 larger forced-flow combinations. All included exact
candidates agreed before timing, and every timed selection was checked again.

Raw artifacts:

- [Native samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv)
- [Completion and omission log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.log)
- [Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)
- [Temporal samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv)

## Fast paths and parallelism

1. Empty valid input returns immediately without allocating a network.
2. Capacity zero validates every row and selects only positive empty intervals.
3. Capacity one delegates directly to `max_weight_non_overlapping`.
4. Empty intervals are handled separately; nonpositive rows are omitted.
5. A start/end sweep computes positive-candidate maximum concurrency. Sufficient
   capacity selects everything useful without building a network.
6. A start-order sweep separates overlap components at `next_start >= max_end`.
   Each component also checks its own concurrency before invoking flow.

The component pilot showed that thread startup loses on small inputs but wins on
substantial independent work. Production uses up to `min(8, available_parallelism)`
scoped standard-library workers when there are at least eight components and
`positive_nonempty_rows * capacity >= 64_000`. Otherwise it stays serial. Workers
own local masks over disjoint batches and restore original indices in batch
order. There is no shared mutable flow network. Endpoint types in the core API
therefore require `Ord + Copy + Sync`; weights still convert losslessly to i128.
The threshold is a practical policy from this machine, not a universal crossover.

## Why the answer is exact

For each distinct endpoint, create a vertex. Consecutive timeline edges have
capacity k and zero cost. Each positive non-empty interval is a forward edge
with capacity one and cost `-weight`. Send k integral units from first to last.

At any cut between consecutive endpoints, interval flow plus timeline flow
equals k, so the selected intervals have concurrency at most k. Conversely, a
feasible interval subset can be placed on k non-overlapping schedules; each
schedule gives a source-to-sink path with timeline edges between its intervals.
Thus minimum cost is the negative of the maximum selected weight. Reverse edges
allow later augmentations to revise earlier selections. Repeatedly fixing a
capacity-1 optimum lacks this ability: `[0,2), [1,3), [2,4), [3,5)` with weights
`3,2,2,3` gives only 8 by repeated scheduling, while capacity 2 admits weight 10.

Removing nonpositive rows cannot lower the optimum; positive empty rows always
increase it without using capacity. Different overlap components share no
capacity constraints, so their optimum objectives add.

## Complexity, memory and arithmetic

For n input rows and constrained component sizes n_c, the worst-case work is
`O(n log n + sum(k * n_c * log(n_c + 1)))`, with `O(n)` additional space including
the mask. The heap can hold O(n_c) entries. Sufficient-capacity cases cost
`O(n log n)`; capacity zero is linear and capacity one uses the existing
`O(n log n)` DP. Parallelism changes wall time, not total asymptotic work.

Serial decomposition retains only one component's network at a time. Parallel
workers additionally copy their disjoint row batches and return selected indices;
total extra storage remains O(n), with at most eight live networks. Generic
adjacency storage allocates per vertex; specialized CSR uses a fixed set of
growing contiguous buffers. No interval conflict graph is constructed.

Signed/unsigned 8-, 16-, 32-, and 64-bit weights are widened exactly to i128.
The core also accepts wider values convertible to i128, including direct i128
test inputs. Cost/potential arithmetic and the final selected objective use
checked operations; overflow returns `IntervalError::WeightOverflow`. Unsigned
u128 reduced distances accommodate large non-shortest detours without signed
wraparound. No float conversion, saturation of objectives or silent weight cast
occurs. All-negative input selects nothing; zero-weight rows are omitted.

## Reproduction and measurement

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity --locked > benchmarks/results/capacity-windows.csv
uv run --locked python benchmarks/capacity_summary.py benchmarks/results/capacity-windows.csv
uv run --locked python benchmarks/capacity_temporal.py > benchmarks/results/capacity-temporal-windows.csv
```

`CAPACITY_BENCH_MAX_N`, `CAPACITY_BENCH_SAMPLES`, and `CAPACITY_BENCH_FAMILY` allow
smaller reproducibility runs. Defaults are 1M rows, three timed samples, and all
families. Candidate order is deterministically shuffled between samples. Each
candidate gets a verified untimed warmup with allocation instrumentation, then
timed runs with allocation counting and phase timers disabled. CSV phase times
come from that separate instrumented call (repeated on the three sample rows),
so they diagnose where work occurs and need not sum to the uninstrumented total.
Zero phase fields mean unavailable for that candidate, not zero work. For the
parallel candidate, `flow_ns` measures the entire worker phase, including local
network preparation and reconstruction; serial candidates separate these phases.

Allocation data counts successful allocations/reallocations and peak live
requested bytes during the whole call, including output and worker buffers.
It excludes inputs, verification, allocator metadata, stacks and process RSS.
Thread/runtime allocation effects are included where they occur within the call.
This is an allocation metric rather than a claim about operating-system peak RAM.

The full suite covers disjoint, sparse (width 3), moderate (width 32), dense giant
cliques, independent 32-row components (peak 16), nesting, staircase/path overlap,
identical intervals, repeated starts/ends, touching endpoints, and many empties.
Every family has sorted and shuffled input. All seven weight distributions
(positive, mixed, zeros, equal, ties, skewed, valuable-long) run at 1K; positive
and mixed run at 10K; positive structural scaling runs at 100K and 1M. Capacities
include 0, 1, 2, 4, 8, 16, 64, and peak-1/peak/peak+1, deduplicated.

Forced-flow work is capped at `n * min(k, peak) <= 2_000_000`. Larger combinations
are explicitly logged as omitted, avoiding hundreds of thousands of shortest
paths for near-capacity giant cliques. This is a bounded stratified suite, not
every size/weight/capacity Cartesian combination. Every retained case compares
every candidate's full optimum and feasibility before accepting any timing.
Full-instance structural oracles check empty/sufficient capacity and clique top-k;
k=1 is independently compared with the existing scheduler. Every workload also
checks all candidates on a ten-row restriction against exhaustive subset search.
Large non-structural instances rely on agreement of the independent flow engines,
not an infeasible full-size brute-force claim.

The temporal benchmark uses independent 32-row cliques with an analytical top-k
oracle at 1K/10K/100K/1M, including near/equal/above concurrency. It measures the
compiled plugin end to end with Int64, Date, Datetime(us), and Datetime(ns, UTC).
Construction and casts occur outside timing; objectives and feasibility are
verified after each evaluation.

## Verification coverage

Deterministic cases cover empty input, every capacity fast path, disjoint/touching
rows, cliques/top-k, nested intervals, duplicates, greedy and repeated-DP
counterexamples, multiple components, signs/zeros, free empties, original order,
invalid rows/lengths and integer boundaries/overflow. Parallel tests compare
worker counts, deterministic reconstruction and error propagation.

Proptest generates up to eleven rows on a small endpoint domain. It checks
feasibility, exhaustive optimality, shape, determinism, capacity monotonicity,
capacity-1 equivalence, sufficient capacity, permutations, translation, negative
row addition, positive empty addition, capacities beyond n, clique top-k and
component additivity. Internal generated networks additionally verify residual
reverse pairs, conservation, capacity bounds, exactly k sink flow, interval-edge
mask reconstruction, cost/objective agreement and deterministic shortest paths.

Python and Rust Polars tests cover integer/Date/Datetime endpoints, all supported
integer weight widths, eager/lazy/filter/window/group execution, chunks, shuffles,
streaming, nulls, dtype rejection, invalid intervals and no broadcasting. Package
checks include explicit capacity and temporal smoke tests outside the checkout.

## Validation results

- `cargo fmt --check`, `cargo test --workspace --locked` (91 tests including
  Rust doctests), and Clippy on all workspace targets with warnings denied: pass.
- `cargo doc --workspace --no-deps --locked` with `RUSTDOCFLAGS=-D warnings`: pass.
- `uv lock --check`, Ruff lint/format, and strict MkDocs build: pass.
- Python tests plus API doctests: 1,086 pass.
- CI/release helper tests: 49 pass; package version/compiler policy check passes.
- The release wheel built from the source distribution passes the same 1,086
  tests and integer/temporal smoke checks in a fresh environment outside the
  checkout, on both Polars 1.44.1 and 1.44.2, Python 3.14.
- Twine strict metadata checks pass for the Windows wheel and source archive.
- Full native and temporal release benchmark runs complete with all checks passing.

Rust Polars tests on this Windows host need `PYO3_PYTHON` set to the environment's
Python executable and the base Python DLL directory on `PATH`. Other platform
wheel builds and Python 3.12/3.13 remain covered by repository CI; they were not
executed locally. No release was published.
