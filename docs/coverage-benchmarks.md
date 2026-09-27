# Exact maximum k-interval coverage

`max_k_coverage` selects at most `k` input intervals maximizing the measure of
their union, then minimizes the selected count among maximum-measure solutions.
The output is a non-null Boolean mask in original row order. All endpoints use
half-open semantics. Empty intervals are valid and never selected. Identical
inputs produce identical masks; the identities chosen under remaining ties are
not part of the API contract.

## Sources and algorithm

Songhua Li, Minming Li, Lingjie Duan and Victor C. S. Lee,
[*Online algorithms for the maximum k-interval coverage problem*](https://doi.org/10.1007/s10878-022-00898-3),
Journal of Combinatorial Optimization **44**, 3364–3404 (2022).
The recurrence was read in the authors' accessible
[preprint](https://arxiv.org/pdf/2011.10938), titled *Online Maximum k-Interval
Coverage Problem*, Section 4.1, Definitions 1–2, Proposition 1, equations (5)–(6).
It is the arbitrary-length offline algorithm, not an online threshold algorithm.

Reuven Cohen and Mira Gonen,
[*On interval and circular-arc covering problems*](https://doi.org/10.1007/s10479-018-3025-6),
Annals of Operations Research **275**, 281–295 (2019), was also investigated
using the [author manuscript](https://u.cs.biu.ac.il/~cohenr5/publications/interval.pdf),
Section 2, Algorithms 1–2. It computes continuous covers of different costs,
then combines them by budgeted weighted independent-set DP, in
`O(n k² + n log n + M)`. It is not implemented here: the additional continuous
cover table, cost loop and reconstruction are unnecessary for physical distance,
where the directly applicable two-family Li recurrence costs `O(kn)`.
No algorithm was inferred from an abstract.

## Adaptation and invariants

These are implementation-specific proofs and indexing conventions. Let `m` be
the number of retained non-empty, non-contained intervals.

1. Validate every original row, even for zero budget, before sorting. Exact
   logical dtype matching and null rejection happen in the Polars adapter.
2. Sort packed `(start,end,original_row)` records by end ascending, start
   descending and row descending. Scan backwards, retaining only strictly
   decreasing starts, then restore ascending order. Both starts and ends are
   now strictly increasing. Duplicates retain one deterministic representative.
3. This dominance rule preserves the lexicographic optimum: replace any selected
   contained row by a retained container. Coverage cannot decrease; selected
   count cannot increase (it decreases if the container was already selected).
   Every discarded row has a retained container. Since the retained rows are
   also input rows, the best attainable coverage is unchanged. Applying the
   replacement to a minimum-count maximum-coverage solution proves equality of
   the secondary optimum as well. This relies on unit selection costs and the
   absence of a public identity preference under ties.
4. For zero-based interval `i`, `p[i]` is the number of preceding intervals with
   `end <= start[i]`. This is the paper's disjoint predecessor expressed as a
   prefix length. If `p[i] < i`, the overlapping predecessor `psi` is index
   `p[i]`: it has the leftmost start of any earlier overlapping interval.
   Otherwise it is absent. Touching belongs to the disjoint case; its overlap
   measure is zero. Strictly increasing starts make the sweep pointer monotone.
   Thus helper computation is `O(m)` **after sorting and dominance pruning**.
   No linear preprocessing claim is made for unrestricted unsorted intervals.

Write `F[j,i]` for the optimum over the first `i` retained intervals with at
most `j` selected, and `G[j,i+1]` for the optimum forced to include interval `i`.
Every value is `(measure,count)`, ordered by higher measure, then lower count.
Adding an interval increments count by one. With `L = end[i]-start[i]`:

```text
disjoint = F[j-1,p[i]] + (L,1)
overlap  = G[j-1,p[i]+1] + (end[i]-end[p[i]],1)
G[j,i+1] = best(disjoint, overlap)
F[j,i+1] = best(F[j,i], G[j,i+1])
```

The overlap branch exists only for `j>1` and `p[i]<i`.
`F[0,*]=F[*,0]=(0,0)`, and `G[1,i+1]=(L,1)`. A forced zero-budget state
is never used. Replacing the preceding overlapping interval by the leftmost
overlapping predecessor cannot worsen a solution; any intervening selected
interval contained in their union is unnecessary. The forced state covers that
predecessor, so the new suffix starts exactly at its end and cannot double-count.
These are the prefix/forced-last transitions from the source.

Unlike the paper's full-prefix shortcut when `i<=j`, we continue to compare
counts, and do not accept redundant rows to fill the budget. Equal objective
values keep the earlier branch. Neither arbitrary floating-point arithmetic nor
coordinate-rank distance is used. Widening **before** subtracting protects
`i64::MIN..i64::MAX` and `0..u64::MAX`. Any union in one supported endpoint
domain has measure at most `u64::MAX`, safely within `i128`.

## Candidate implementations

All experimental code lives in `crates/intervals-core/benches/support/coverage.rs`
and is also compiled into the oracle/property tests. Only the selected kernel
is linked into the public library.

| Candidate | Layout / computation |
| --- | --- |
| A `full` | Full `F` and `G` tables of `(i128,usize)` scores and full byte decisions |
| B `rolling` | Four objective rows and one byte per state containing two decision bits |
| C `recompute` | Four objective rows and one decision row; replay earlier budget layers after each accepted interval |
| D1 `binary` | B with binary search on end coordinates |
| D2 `rolling` | B with monotone sweep on the retained records |
| E `indirect` | B with sorted original indices, loading endpoints indirectly throughout preprocessing and DP |
| `unpruned` | A on all non-empty rows; binary disjoint lookup and a prefix skyline stack for overlapping predecessor queries |
| `components` | Per-component coverage curves, global budget convolution, then per-component reconstruction |
| `production` | Public core, including validation and fast paths; no internal phase clocks |

A and B take `O(km)` DP time. A uses about `64(k+1)(m+1)` score bytes
plus decisions on this 64-bit build. B uses `128(m+1)` score bytes plus `km`
decision bytes and row headers. The two bits in each decision record encode
prefix accept/skip and forced-state disjoint/overlap. Keeping bytes avoids bit
indexing machinery. Reconstruction follows these choices in `O(m+k)` time.
C is exact and has `O(m)` working memory but worst-case `O(k²m)` time; it is
retained only as a benchmark experiment, not an additional public option.

Component curves are combined by enumerating budget splits, with the same
lexicographic score. The experiment re-solves each chosen component to obtain
its mask. This tests the requested decomposition without adding a second
production reconstruction mechanism. Whole-instance DP already handles gaps
through its disjoint transition. Component decomposition is not used in production.

## Fast paths

- Empty input and `k=0` return an empty/all-false mask after validation.
- `k=1` scans for the unique or first longest positive interval in `O(n)`,
  without sorting or DP. An all-empty input selects nothing.
- Dominance pruning removes empty, duplicate and contained rows by the proof above.
- After sorting, compute a minimum representation of the **complete union**:
  at each covered frontier, choose the farthest-reaching available interval
  whose start is at or before it; restart at each gap. Exchanging the next
  interval for the farthest-reaching choice never increases the required
  number of future intervals. Thus the resulting count is globally minimal.
  If this count is within budget, return this mask. This also handles `k>=n`
  and early saturation. Otherwise discard the cover and use DP.

The production bound is `O(n log n + k m)` time and `O(n + km)` additional
space, including the output; a DP run always has `k < m <= n`. This implies
the advertised worst-case `O(n log n + min(k,n)n)` time and
`O(n + min(k,n)n)` space. Temporal kernels share the same physical integer
algorithm: Date measures days and Datetime measures its ms/us/ns ticks,
including timezone-aware columns.

## Correctness gates

The independent test oracle enumerates every subset of size at most `k`, sorts
selected intervals by start, merges overlap/touching, and sums segment lengths
in `i128`. It compares measure and minimum count, never identity under ties.
An additional `O(kn²)` reference enumerates every possible preceding selected
interval, without pruning or phi/psi. It is first checked against brute force,
then against every candidate on generated collections of 12–64 intervals.
The benchmark also independently merges every candidate's returned mask before
timing; all candidates must agree with A/B's stored objective. Small benchmark
fixtures additionally pass the exhaustive oracle.

Named regressions cover the two greedy failures, zero/one/oversized budgets,
empties, disjointness, touching, nesting, duplicates, equal starts/ends,
budget-sensitive optima, early saturation, input order and extreme integers.
Seven integration proptests use 384 cases each, generating up to 11 intervals
in `[-8,12]`, plus structural families. Together they exercise the requested
20 properties: cardinality, objective/count, length, determinism, monotonicity,
saturation, zero/one budgets, permutation, translation, positive scaling,
arbitrary/empty/duplicate insertion, disjoint/nested/duplicate families,
component convolution, full-union and sum-of-lengths bounds. An eighth proptest
compares binary/sweep helpers with quadratic definitions; a ninth exercises the
independent quadratic DP on larger instances. Production unit
properties check skyline dominance and both helpers, and bypass all fast paths
to compare reconstruction with stored DP values. A continuous-union fixture
also cross-checks `minimum_cover`.

Native plugin tests cover eager/lazy select, with_columns, direct filters,
expressions, windows, group aggregation, multiple chunks, all eight integer
dtypes, Date, all Datetime units, UTC/Helsinki zones, DST physical distance,
dtype/null/shape errors, extremes and random exhaustive-reference cases.

## Benchmark protocol

Run release builds only:

```sh
cargo bench -p intervals-core --bench max_k_coverage --locked > benchmarks/results/coverage-core.csv
# With the locally built release wheel installed in an external environment:
python -I /path/to/checkout/benchmarks/coverage_temporal.py
```

The fixed seed is `20260927`. Every method has a correctness invocation, a
separate allocation invocation, then three timed invocations. Inputs and
oracles are outside timing. Total time includes allocation and temporary
destruction, excluding destruction of the returned result. Sort includes
record construction and dominance pruning. Preprocess measures helper arrays;
DP and reconstruction have separate clocks. Component-local helpers are included
in that candidate's DP/reconstruction phases. The production phase columns are
zero because the public function is deliberately not instrumented.

Allocation statistics use the existing counting allocator in a separate,
untimed call: **peak live requested heap bytes**, excluding inputs, verification,
stack, allocator overhead and OS RSS. This is not a process-memory measurement.

The matrix includes 1K, 10K, 100K and 1M rows and budgets
`0,1,2,3,4,8,16,32,64`, plus `k=n` at 1K. Supplemental empty and eight-row
cases time empty input and oversized budgets for every method.
It covers disjoint, identical, nested,
dense/sparse random overlap, touching, staircase, equal starts, equal ends,
duplicates, empties, dominated intervals, separated components, redundant long
overlap, complementary shorter intervals, budget-sensitive and saturation
families. Each has end-sorted, reversed and shuffled order. At 1M, use five
representative families (disjoint, dense, staircase, dominated, components)
and budgets `0,1,8,64` to keep the total run practical.
Full/unpruned tables are capped at four million `nk` cells; replay at 30 million
`nk²` updates; component convolution at 10K rows and `k<=64`. These explicit
safety limits leave every implementation benchmarked at meaningful sizes.

The native release-wheel matrix covers Int64, Date and Datetime[us], four sizes,
three structures, two orders and budgets `0,1,2,8,32,64`; other temporal units
and timezone metadata are checked for correctness rather than multiplying the
performance matrix. Known structural optima certify each native workload.

## Native Polars comparison

The installed Polars API and the current
[expression reference](https://docs.pola.rs/api/python/stable/reference/expressions/index.html)
and [window documentation](https://docs.pola.rs/user-guide/expressions/window-functions/)
were investigated. No equivalent exact interval-budget optimization was found.
Sorting, cummax, and shifts can measure a *given* union, but choosing its members
under a global budget requires DP across alternatives. A subset enumeration or
Python UDF would not be a comparable native expression baseline. No speedup
against an invented dataframe baseline is reported.

## Recorded results

Measured on 2026-09-27 on Windows 11 x86-64, AMD Ryzen 9 3900X (12 cores,
24 logical processors), approximately 32 GiB RAM, Rust 1.98.1, Python 3.14.0,
Polars 1.44.2 and uv 0.11.3. The final core and native runs were sequential,
without concurrent compilation or tests. Values below are medians of three
release samples; MiB means 1,048,576 bytes.

The core run contains **33,246 samples**, covering 1,506 input/budget workloads
and 11,082 candidate/workload combinations. The installed-plugin run contains
**1,296 samples** over 432 workloads. All correctness gates passed.

- [Core raw samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-core.csv)
- [Core medians](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-summary.csv)
- [Installed-plugin raw samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv)
- [Environment, source/artifact hashes and validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json)

### Layout and reconstruction comparison

Shuffled staircase, `n=10,000`, `k=32`; all intervals survive dominance pruning:

| Implementation | Total ms | Peak heap MiB | Allocations |
| --- | ---: | ---: | ---: |
| A: full tables | 4.469 | 20.988 | 117 |
| B: rolling rows + decisions | 2.455 | 2.073 | 57 |
| C: replay reconstruction | 3.189 | 1.767 | 21 |
| D1: binary helpers + B | 2.549 | 2.073 | 57 |
| E: indirect records + B | 2.903 | 1.823 | 57 |
| Unpruned full tables | 5.193 | 20.988 | 130 |
| Component curves + merge | 4.664 | 2.313 | 73 |
| Production | 1.707 | 1.988 | 62 |

B's phase medians here were 0.270 ms sorting/pruning, 0.022 ms helpers,
2.155 ms DP and 0.006 ms reconstruction. The raw data retains every phase.
Phase medians need not sum to the total median, which also includes temporary
destruction and bookkeeping. Production specializes the retained-record
invariants, stores only `phi` (deriving `psi`), and includes validation and the
full-union check; it is not merely the instrumented benchmark wrapper.

**Production choice: B, packed records, monotone helper sweep.** Full tables
remain the readable reference candidate, but their large objective storage
does not improve reconstruction. Replay is clean and exact, but its tradeoff
is poor for the default: on sorted disjoint `n=1,000,k=64`, B took 0.399 ms
and 0.225 MiB; replay took 7.620 ms and 0.163 MiB. On the larger staircase
`n=100,000,k=16`, replay took 17.196 ms versus B's 15.837 ms while saving
1.526 MiB. Replay remains experimental code only.

### Helper computation and record locality

At `n=1,000,000,k=8` on a sorted staircase, helper computation took 5.034 ms
with the sweep and 18.186 ms with binary search; total times were 124.826 ms
and 137.504 ms. With shuffled input, helper times were 5.579 and 18.548 ms,
but total times were 188.449 and 176.908 ms respectively. Lower helper cost
therefore did not guarantee a lower total in every measured case. The sweep
is selected for its simple monotone invariant and consistently lower helper
cost on these large cases, not a claim that binary search always loses.

Indirect records used 154.866 MiB versus packed B's 170.866 MiB on that
million-row workload. They were slightly faster when sorted (116.596 versus
124.826 ms), but much slower when shuffled (598.980 versus 188.449 ms).
Packed records avoid repeated scattered endpoint loads during DP, so that
layout is the default despite its additional record storage.

### Scaling in input size, budget and memory

Production staircase times, in milliseconds:

| n | Sorted k=8 | Shuffled k=8 | Sorted k=64 | Shuffled k=64 |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 0.046 | 0.091 | 0.293 | 0.313 |
| 10,000 | 0.448 | 0.662 | 2.909 | 3.227 |
| 100,000 | 8.056 | 10.715 | 36.603 | 38.739 |
| 1,000,000 | 120.908 | 161.349 | 648.371 | 669.151 |

Both the number of DP updates and memory grow with the retained interval
count and budget; crossing cache sizes also affects these timings. The table
does not imply perfectly linear wall-clock scaling.

Peak heap on the 100K-row sorted staircase:

| k | A full MiB | B rolling MiB | Production MiB | Production ms |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 16.924 | 17.019 | 0.095 | 0.146 |
| 8 | 60.317 | 17.687 | 16.829 | 8.056 |
| 16 | 109.909 | 18.450 | 17.592 | 11.960 |
| 32 | 209.093 | 19.976 | 19.118 | 19.975 |
| 64 | not run: table cap | 23.029 | 22.170 | 36.603 |

For DP runs, each additional budget layer adds `m` decision bytes plus a
24-byte vector header on this host. The four score rows stay fixed. Thus
production grows by about 3.052 MiB from `k=32` to `64` when `m=100,000`.
At one million retained intervals, production used 162.283 MiB at `k=8`
and 215.690 MiB at `k=64`. The output and record allocation remain `O(n)`
even when pruning makes `m` small. The `k=1` row bypasses these buffers.

### Fast paths and component experiment

The validated longest-interval scan is retained: on the shuffled 100K
staircase, `k=1` took 0.150 ms versus 7.343 ms for general B. Zero budget
and empty-input cases are included in the raw matrix. Oversized budgets are
also timed; they do not cause oversized DP allocations in production.

Dominance pruning is retained. For 100K shuffled nested intervals at `k=32`,
unpruned A took 66.513 ms, pruned B 3.865 ms, and production 3.957 ms.
For many duplicates/contained rows, sorting and validation can dominate, so
the fast paths are not claimed to accelerate every fixture.

The minimum full-union cover fast path is retained. In the shuffled 100K
saturation fixture at `k=64`, B took 57.444 ms and 23.029 MiB; production
took 4.288 ms and 3.095 MiB, selecting only two intervals. It remains exact
for both objectives. No heuristic pruning or local longest-first selection
is used for unsaturated budgets.

Component decomposition has mixed results. On 10K sorted rows in separated
16-interval components, `k=64` took 2.647 ms and 0.738 MiB with decomposition,
versus B's 4.707 ms and 2.379 MiB (production: 3.104 ms, 2.293 MiB).
At `k=8`, decomposition took 1.112 ms versus B's 0.671 ms. On 10K completely
disjoint rows at `k=64`, it took 7.503 ms and 5.971 MiB versus B's 3.288 ms
and 2.379 MiB. The extra component curves, convolution and reconstruction
are not justified as a general production path by these measurements;
the exact experiment remains available in the benchmark suite.

### Installed Polars and temporal endpoints

Million-row staircase, complete lazy query collection, milliseconds:

| dtype | Sorted k=8 | Shuffled k=8 | Sorted k=64 | Shuffled k=64 |
| --- | ---: | ---: | ---: | ---: |
| Int64 | 125.259 | 160.872 | 638.936 | 680.926 |
| Date | 117.395 | 139.255 | 618.049 | 630.053 |
| Datetime[us] | 125.698 | 164.326 | 642.025 | 688.737 |

Date uses a narrower physical integer, while Int64 and Datetime use the same
width. These are measurements of equivalent physical-coordinate patterns,
not evidence that one logical dtype is universally faster. This matrix also
includes disjoint and identical intervals, all four sizes, and zero/one budgets.

### Validation and limits

The final workspace run passed **214 Rust tests and doctests**. The Python
suite passed **1,560 tests and doctests**, both in the checkout and against
the installed release wheel from an external directory. New feature coverage
includes 30 core integration tests (nine proptests with 384 cases each), three
production unit tests, two Rust Polars integration tests and 143 Python tests.
Release/CI helper checks passed 49 tests; plotting checks passed three tests
and eight subtests.

`cargo fmt --check`, workspace tests, Clippy with warnings denied, Rustdoc
with warnings denied, `uv lock --check`, Ruff lint/format checks, strict MkDocs,
local wheel/sdist metadata checks, source inclusion and strict Twine checks
passed. The existing benchmark figure generator also completed. The local
release wheel was built with maturin in release mode; the source archive was
regenerated after the feature changes.

These results describe one Windows host, synthetic data and three samples
per case. Memory is requested heap accounting, not OS RSS. The local package
check covers CPython 3.14 on Windows; the full cross-platform wheel matrix and
source-installation matrix remain CI checks. No universal speedup or equivalent
native-Polars baseline is claimed.
