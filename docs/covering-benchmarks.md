# Exact interval covering: engineering and measurements

The production APIs are `minimum_cover(start, end, *, target_start, target_end)`
and `minimum_cost_cover(start, end, *, cost, target_start, target_end)`. Both return
Boolean masks in original row order. Rust Polars exposes corresponding functions
taking endpoint/cost `Series` and two `&Scalar` targets. The independent core
accepts endpoint slices, two scalar endpoints, and (for weighted covering) a cost
slice. It has no production dependencies, including Polars, Arrow, Python or chrono.

## Exact semantics and algorithms

All intervals and the single target are half-open. Touching intervals chain.
Empty targets return all false after validation. Empty intervals never help;
outside rows are discarded after validation, and useful rows are clipped to the
target. Reversed targets/intervals and null endpoints are errors. A non-empty
target without a continuous cover raises
`target interval cannot be covered by the supplied intervals`.

**Minimum cardinality:** sort packed `(effective_start, effective_end, row)`
records and linearly sweep. Among every interval beginning at or before the
current frontier, choose the furthest end. Every scanned alternative ends at or
before that new frontier and can be discarded. An exchange argument establishes
optimality: replacing an optimal cover's first advancing interval with this
furthest-reaching interval never increases the required remaining intervals.
Repeat on the remaining target suffix. Equal effective ends prefer the original
row index.

**Minimum cost:** sort candidates by effective right end, compress right ends
plus target start, and maintain the best `(cost, count, frontier_index)` at
reachable frontiers. A reversed Fenwick tree queries the minimum over reachable
frontiers `x >= l`. Only frontiers strictly below `r` have been published when
processing `[l,r)`. Add the interval's cost and one selected interval. Publish
the best state only after all candidates ending at `r` have been queried.
Backpointers reconstruct the original row mask.

This recurrence is exact: any advancing interval extends a reachable continuous
prefix, and any nonredundant cover can be ordered by its advancing right
endpoints. Nonnegative costs and the secondary count objective ensure that a
nonadvancing interval is never necessary. The cheapest local interval is not a
valid greedy rule: `[0,4):1`, `[0,6):5`, `[4,10):100`, `[6,10):5` has optimum 10,
although starting with the cheapest interval can cost 101.

Weighted comparisons first minimize cost, then count; remaining ties use row
index and predecessor coordinate. Deterministic masks are guaranteed for identical
input, but a particular tied mask is not a stable public contract. Costs must
be nonnegative integers. Python/Polars accepts the eight integer dtypes through
64 bits, without nulls or casts. The core accepts integer types convertible to
`i128`. Addition is checked; overflowing paths cannot return to a representable
cost because costs are nonnegative. Such paths are discarded. If the final state
is absent after an overflow, an unweighted feasibility check distinguishes
infeasible coverage from an optimum exceeding `i128::MAX`.

Both production algorithms take **O(n log n) time and O(n) additional space**,
including output. The greedy sweep and weighted reconstruction are linear after
sorting/DP. For `m` useful rows and `q <= m+1` distinct frontier coordinates,
weighted storage is `O(m+q+n)`: candidates, coordinates, one Fenwick tree,
backpointers and the output mask. The tree is released before allocating the
output mask. The Polars adapter borrows contiguous physical endpoint slices;
multiple chunks require copying. Costs are widened once to `i128`.

## Candidates and production selection

| Candidate | Implementation | Retained where |
| --- | --- | --- |
| MC-A | Packed candidates, start sort, linear greedy sweep | Production; instrumented benchmark |
| MC-B | Original-index sort, indirect greedy sweep | Tests/benchmarks |
| MC-C | Start sort, max-heap reference | Tests/benchmarks |
| MC-A-detect | MC-A with an explicit linear sortedness check | Tests/benchmarks |
| MCC-A | Reversed Fenwick suffix-min frontier DP | Production; instrumented benchmark |
| MCC-B | Iterative segment-tree range-min frontier DP | Tests/benchmarks |
| MCC-C | Direct scan of prior reachable frontier states | Tests/benchmarks; at most 1K rows |

Both trees stop updating ancestors when the stored minimum does not improve.
The segment-tree candidate does not retain an unnecessary second DP array.
There are no production method switches or graph/solver dependencies.

Packed candidates give the stronger general-purpose cardinality implementation
on shuffled data. Indirect sorting saves candidate memory and can win on sorted
or repeated geometry, but pays for scattered endpoint access on varied shuffled
input. The heap is useful as a distinct cross-check, not a production improvement.
Fenwick has lower storage and wins the important workloads with many reachable
frontiers. Segment-tree performance is competitive on small/compressed or
infeasible instances but does not justify its extra storage in production.

**No explicit sorted-input fast path is retained.** The standard library sort
already handles sorted input efficiently. The additional scan gives only modest,
inconsistent benefits and sometimes costs time on equal-start data. Sorted,
nearly sorted and shuffled measurements are all retained, including losing cases.

## Recorded results

Measured on Windows 11 x86-64, AMD Ryzen 9 3900X (12 cores / 24 threads),
Rust 1.98.1, Python 3.14, Polars 1.44.2, using the optimized Cargo bench profile.
The final candidate run contains **7,425 timed samples across 396 workloads**
(2,475 candidate/workload combinations), with three repeats each. The maximum
measured candidate allocation peak is 157.83 MB (decimal MB, not RSS).

Median per-workload runtime ratios (candidate / corresponding packed or Fenwick
baseline; less than one is faster):

| Candidate | Sorted | Nearly sorted | Shuffled |
| --- | ---: | ---: | ---: |
| MC-A-detect / MC-A | 0.944 | 0.984 | 1.000 |
| MC-B / MC-A | 0.880 | 1.015 | 1.339 |
| MC-C / MC-A | 1.662 | 1.458 | 1.207 |
| MCC-B / MCC-A | 1.237 | 1.209 | 1.122 |
| MCC-C / MCC-A, 1K only | 1.254 | 1.250 | 1.141 |

Each geometry/order/cost/size workload receives equal weight in these medians.
Sorted/shuffled include the additional physical-width runs; nearly sorted does
not. The quadratic median includes very small compressed state spaces, which
conceals its poor scaling on dense/equal-start data: at just 1K rows its median
is 1.51 ms on dense data and 2.25 ms on equal starts, versus 0.089 ms for Fenwick
on the shuffled dense/random case.

Representative **1M-row shuffled** medians, milliseconds:

| Workload | MC-A | MC-B | MC-C | MCC-A | MCC-B |
| --- | ---: | ---: | ---: | ---: | ---: |
| Touching chain | 50.07 | 84.18 | 54.84 | 188.08 | 205.33 |
| Dense overlaps | 47.48 | 65.37 | 64.24 | 226.50 | 389.63 |
| Equal starts | 13.85 | 8.99 | 30.05 | 198.61 | 274.19 |
| Mostly irrelevant | 0.91 | 0.93 | 0.88 | 2.05 | 2.04 |
| Date-width chain | 38.79 | 72.23 | 51.50 | 210.87 | 195.95 |
| Date-width dense | 47.05 | 79.82 | 69.78 | 275.19 | 448.58 |

The Date-width chain is a real segment-tree win; the production choice is based
on the wider workload results and storage, not universal dominance. For the
1M-row i64 chain, packed/indirect candidates peak at 26.17/9.39 MB respectively;
Fenwick/segment tree peak at 105.17/157.83 MB. For dense i64 input, the weighted
peaks are 87.17/151.83 MB. Irrelevant-row workloads need about 1 MB, mostly output.

For the selected algorithms on a 1M-row shuffled chain, median packed
preprocessing/sweep times are 45.95/3.09 ms; Fenwick preprocessing/DP/reconstruction
times are 50.26/131.92/3.60 ms. Dense Fenwick phases are 50.28/172.78/0.063 ms.
The chain uses 20 allocation/reallocation events for packed greedy and 23 for
Fenwick. Raw samples retain all phase and memory data, including failures.

Sorted detection's benefit is too inconsistent to justify an extra pass: the
1M sorted chain changes from 13.86 to 14.11 ms and the sorted Datetime-width dense
case from 14.26 to 15.28 ms. Both improve dramatically over shuffled input already
through the standard sort, without a separate production fast path.

Raw artifacts:
[candidate samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv),
[completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log),
[environment and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json),
[native temporal samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv).

The raw measurements retain their original source hashes. Separate cleanup
verification in the environment metadata records the consolidated harness and
adapter sources; the measured solver and candidate implementations are unchanged.

The native **release wheel installed outside the checkout** produced 1,536
samples across 512 dtype/workload combinations. Representative 1M-row shuffled
chain medians include all plugin/validation/adapter overhead:

| Endpoint dtype | minimum_cover (ms) | minimum_cost_cover (ms) |
| --- | ---: | ---: |
| Int32 | 39.99 | 196.09 |
| Int64 | 69.03 | 210.80 |
| UInt64 | 52.47 | 208.29 |
| Date | 39.83 | 196.34 |
| Datetime(ms) | 52.91 | 213.34 |
| Datetime(us) | 52.61 | 206.46 |
| Datetime(ns, UTC) | 52.58 | 221.16 |
| Datetime(ns, Europe/Helsinki) | 51.64 | 205.19 |

The native data and costs differ from the core candidate harness, so these are
end-to-end measurements, not overhead estimates obtained by subtracting tables.

## Reproduction and workload coverage

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-windows.csv
uv run --locked python benchmarks/covering_summary.py
# After installing a locally built release wheel:
python benchmarks/covering_temporal.py > benchmarks/results/covering-temporal-windows.csv
```

`COVER_BENCH_MAX` and `COVER_BENCH_SAMPLES` optionally limit size and repeats;
defaults are one million rows and three timed samples. The candidate harness
uses a fixed seed (20260926), a verified allocation-measurement warmup, then
randomized candidate order for each timed repetition. Construction, input
generation, shuffling and independent verification are outside timed regions.
The timings include internal phase clocks and output destruction is excluded.

Sizes are 1K, 10K, 100K and 1M. Seventeen geometry/difficulty families cover
single-interval covers, exactly two intervals, mostly disjoint rows, touching
chains, dense overlaps, nested intervals, duplicates, equal starts, equal ends,
far-outside endpoints, irrelevant rows, empty rows, one giant component,
beginning/middle/end gaps, and one expensive long interval versus cheap shorts.
Orders are sorted, nearly sorted (1% adjacent swaps), and shuffled.

Dense and duplicate families exercise equal positive, mostly zero, small random,
skewed, different duplicate costs, and many ties. Their `expensive` label is an
additional unit-cost control; the expensive-long family supplies the actual
expensive-long-versus-cheap-short comparison. Other families use small random
costs. This is a bounded matrix, not a full Cartesian product of every
distribution and geometry.

The core additionally benchmarks Date's `i32` physical width and Datetime's
`i64` physical width on chains, dense overlaps and duplicates, sorted/shuffled,
at all four sizes. The separate **native release-wheel** suite benchmarks actual
Int32, Int64, UInt64, Date, Datetime(ms), Datetime(us), Datetime(ns, UTC), and
Datetime(ns, Europe/Helsinki) columns, including strict scalar configuration,
physical adaptation and plugin dispatch. It uses chains, dense overlaps,
duplicates and irrelevant rows, all four sizes, sorted and shuffled input.

Each candidate's objective must match production, and each successful mask must
independently cover the target before timing is accepted. The different DP
implementations cross-check larger instances; the exponential oracle is bounded
to small correctness instances, and the quadratic candidate to 1K benchmark rows.
The native suite checks coverage on physical coordinates and deterministic masks.

## Timing and memory interpretation

Raw CSV columns retain every sample's total, preprocessing (validation, clipping,
sorting/compression), optimization (sweep/DP), and reconstruction time. Greedy
selection writes the mask during its sweep; its reconstruction time is zero.
The weighted mask is reconstructed in a separate measured phase. Total time also
includes temporary-buffer destruction and phase-clock overhead, so it need not
equal the sum of phase times. Early failures legitimately skip later phases.

Peak live requested allocation bytes and allocation/reallocation event counts
are measured in a **separate untimed invocation** using the system allocator.
These include algorithm buffers and output, but exclude caller-owned inputs,
oracle work, allocator metadata, stacks and process RSS. Measurements are serial;
neither solver introduces internal workers.

This is synthetic evidence from one Windows x86-64 machine, not a cross-platform
speed guarantee. Short timings are sensitive to scheduler and clock noise.
Use per-workload medians, not individual minima, to compare methods.

There is no ordinary native Polars expression baseline: choosing a global subset
requires an iterative frontier algorithm or dynamic programming with
reconstruction. An overlap count, group aggregate, join, or sorted union is not
an equivalent minimum cover. No misleading substitute is timed.

## Correctness and integration evidence

The independent test oracle enumerates all subsets and checks continuous coverage
by sorting selected original intervals and extending a frontier. It compares
objectives, not masks under ties. Proptest generates 0–11 intervals from endpoint
coordinates −8 through 12, costs 0–20, and independent valid targets, including
empty targets. Both production solvers and all six primary candidates are checked.

Properties cover feasibility/failure, exact objectives, mask length, no empty
selections, determinism, permutation and translation invariance, monotonicity on
adding candidates, irrelevant-row invariance, single-cover bounds, positive cost
scaling, zero-cost cardinality and unit-cost equivalence between algorithms.
Additional generated tests compare Fenwick suffix and segment-tree arbitrary
range queries with naïve minima, and every reachable reconstructed DP prefix
with a subset optimum restricted to processed endpoints. Backpointers must
strictly advance; unreachable states stay unreachable. Regressions cover equal
right ends, both greedy traps, duplicate costs, and exact `i128` overflow behavior.

Rust Polars and native Python tests exercise all supported endpoint/cost integer
widths, Date, all Datetime units and supported timezone metadata, exact target
conversion, eager/lazy/filter/window/group aggregation, streaming, multiple
chunks, original row order, invalid intervals, nulls and unsupported cost types.
Python also runs a second seeded subset oracle through the compiled plugin.

The completed local validation includes 99 Rust tests/doctests, 1,229 Python
tests/doctests (also all passing against the installed external release wheel),
and 49 CI/release helper tests. Formatting, Clippy across all targets with denied
warnings, Rustdoc with denied warnings, the uv lock check, Ruff lint/format and
strict MkDocs builds pass. A release wheel and source archive were built and
audited for metadata, license, type marker, native extension and new source files.
The full 15-wheel cross-platform CI matrix is not a local test and was not run.
