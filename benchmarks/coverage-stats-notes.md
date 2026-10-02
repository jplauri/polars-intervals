# Coverage statistics algorithm selection

The production function prepares each source collection once. It preserves raw
nonempty starts and ends for counts. It reuses `merge_sorted` for source union
runs and stores exact `i128` cumulative lengths. Queries remain separate rows.

For a nonempty query `[a,b)`, every nonempty source ending at or before `a`
also starts before `b`. Subtracting that end count from the count of starts
strictly before `b` leaves exactly the overlapping source rows. Duplicates remain
in both count arrays. Empty queries bypass this formula and return zero counts.

For boundary `x`, completed union runs supply their cumulative lengths. At most
one following run contributes a partial length. Their sum is `F(x)`, and
`F(b)-F(a)` is covered length. The sealed endpoint domain bounds a whole union
by `u64::MAX`. All subtraction happens after widening to `i128`. Fractions cast
both final lengths to `f64` and divide. They do not subtract floating prefixes.

## Retained rule

Let `n` be original query rows, `m` original source rows including empty rows,
and `p` canonical nonempty source union runs. Both operands validate before any
selection or sortedness checks.

1. If `n >= m` and both original query endpoint arrays are actually sorted,
   use four monotone cursors directly in query order. No request arrays are needed.
2. Otherwise, if `n <= m` and `p >= 32`, use packed offline boundary requests.
   Sort query starts and ends independently, scan, and restore original rows.
3. Otherwise, use per-query binary searches with a preallocated output buffer.

The cutoff of 32 is empirical and conservative. This rule does not guarantee
the fastest route on every input or machine. It avoids sorting many queries
against a much smaller source collection. Sorting occurs only when `n <= m`,
so the worst-case bound remains `O(m log(m+1) + n log(m+1) + n)`. Additional
storage is `O(m+n)`, including output. All routes have identical exact semantics.

The private benchmarks retain pure binary searches and packed sweeps. They call
the same production preparation and evaluation helpers. There is no public
algorithm option or reusable index. The direct ordered scan is the sorted-input
specialization of the offline sweep.

## Evidence and limits

Each CSV is a separate run. Do not pool medians or relabel the original binary
timings as measurements of the later preallocated or selected production code.
Metadata and source archives share each CSV's prefix in `benchmarks/results/`.

- `coverage-stats-core-local.csv`: initial complete A/B experiment. Seven sizes
  from zero through one million, two seeds, five samples, and two warmups.
  It includes Int64, UInt64, and Int16 where generated coordinates fit.
- `coverage-stats-core-crossover.csv`: Int64 10k/100k comparisons, seven samples,
  two seeds. Adds controlled union-run counts and ordered/hybrid experiments.
  The experimental hybrid used `p > 1` and `n >= m`. It is not the retained rule.
- `coverage-stats-core-million-crossover.csv`: one-million-row Int64 follow-up,
  five samples and two seeds. The ordered candidate preallocates output.
  On unordered inputs it is pure preallocated binary search. The provisional
  hybrid still used `p > 1` and `n >= m`, which exposed query-heavy losses.
- `coverage-stats-core-production.csv`: final selected production, preallocated
  binary search, and packed sweeps. Seven representative cases at 1k, 100k and
  one million query rows, Int64, two seeds, five samples and two warmups. Its
  metadata reports unchanged source hashes during measurement. The accompanying
  `coverage-stats-core-production.sources.zip` preserves the measured core,
  benchmark runner, tests, Cargo inputs and exact selection rule.

The final selected production run, seed 7 with one million queries and sources,
measured 58.4 ms for sorted disjoint inputs, 263 ms for shuffled sparse inputs,
and 227 ms for duplicate sources. The corresponding pure preallocated binary
times were 195 ms, 1044 ms and 477 ms. The final rule retained a shuffled genomic
loss: 400 ms versus 271 ms with packed sweeps. With one million shuffled queries
and ten thousand sources, production took 76.1 ms versus 201 ms for packed
sweeps. These final measurements remain separate from the experiments below.

The final source archive SHA-256 is
`abf58740c4fcd27ee37bd01a07a52add2116e282deaa33f2f2733c1b714d7964`.
The measured `src/coverage_stats.rs` SHA-256 is
`dafd958ed080ce1bfa543465df98b1c598fabeea51d695da004cf4b62ad9eda3`.

In the million-row follow-up, seed 7, sorted disjoint queries and sources took
69.0 ms with direct cursors and 86.0 ms with packed sweeps. Shuffled sparse
queries took 1136 ms with preallocated binary search and 276 ms with packed
sweeps. Duplicate sources took 687 ms and 276 ms respectively. These are
underlying algorithm times, not complete Polars calls.

Losses matter. For one million shuffled queries against ten thousand sources,
32 union runs took 141 ms with preallocated binary search and 214 ms with packed
sweeps. A single long union run took 141 ms versus 252 ms at balanced sizes.
Conversely, shuffled synthetic genomic queries with one union run favored
packed sweeps, 308 ms versus 543 ms. The conservative rule retains that loss
rather than introducing more data-specific switches. Two-run balanced cases
had overlapping sample ranges and did not establish a dependable crossover.

Memory is peak requested live heap from a separate untimed allocator call.
It includes preparation, output and request buffers. It excludes caller inputs,
allocator bookkeeping, stack and process RSS. Packed sweeps add two query
request arrays. For the initial one-million-row disjoint case, this metric was
115.9 MB for the original binary implementation and 146.3 MB for packed sweeps.
Output preallocation subsequently reduced the binary/direct output capacity.

Small cases compare every retained route with an original-source elementary-cell
oracle. A separate integer-tick oracle cross-checks that oracle. Prefix tests
check every relevant boundary directly. Extreme Int64/UInt64 spans, `i8` widths,
and huge-prefix cancellation are exact tests. Larger generated tests use bounds,
permutations and duplicate-source transformations. Proptest case counts follow
`PROPTEST_CASES`; the development run used 4096 cases per property. Larger
benchmark fixtures use structural checks, direct source counts at sampled rows,
and whole-output candidate agreement. Candidate agreement alone is not an
independent validation of their shared reduction.

These are synthetic fixtures on one Windows machine. Full Polars measurements,
payloads, groups, scans and temporal metadata are separate benchmark scopes.
