# Benchmark algorithm notes

Supporting derivations, validation details and development experiments for the
compact [published reports](https://jplauri.github.io/polars-intervals/benchmarks/).
Measurements below describe the recorded revisions; raw samples and metadata remain
the authoritative evidence. Shared timing and memory definitions live in the
[benchmarking guide](https://jplauri.github.io/polars-intervals/benchmarking/).

## Nesting depth

### Recurrences and sorted-input investigation

| Candidate | Approach | Role |
| --- | --- | --- |
| Packed Fenwick | Packed `(start,end,index)` records, compressed ends, prefix maximum over reversed end ranks | Exact candidate |
| Indirect Fenwick | Sort row indices and access the original endpoint arrays; same max recurrence | Exact locality/copying comparison |
| Iterative segment tree | Packed sort and compressed ends; iterative maximum query and point update | Exact alternative and cross-check |
| Vector frontier | Greatest achievable ending endpoint for each chain length | Production with direct append; binary-search-only comparison retained |

All candidates sort by start ascending, then end descending, then original index.
Every strict container precedes its child. Equal starts with different ends must
remain visible to one another; batching the entire equal-start run would be wrong.
Instead, process **only identical `(start,end)` geometries atomically**: determine
the common depth before inserting that geometry. Assign the result to every
original row in the batch. Equal ends at later starts are strict steps too.

The Fenwick recurrence uses reversed end ranks so `previous.end >= current.end`
is an inclusive prefix. The maximum stores chain **cardinality**, with zero
explicitly meaning no predecessor; a present row at depth zero stores one.
The returned depth is the maximum preceding cardinality. This avoids confusing
an absent predecessor with an outermost predecessor.

The frontier stores `tails[d]`, the **largest** end achievable by any processed
chain of cardinality `d+1`. These endpoints are nonincreasing: removing a last
member gives a shorter chain with an end at least as large. For a new geometry
ending at `e`, the number of tails `>= e` is the maximum predecessor cardinality.
Replace the following tail with `e`, or append if all tails qualify. Earlier
tails are already at least `e`; later tails cannot improve from a chain this
short. A single binary search and one update therefore preserve the invariant.
This is the monotone breakpoint idea in a contiguous vector; no ordered map,
pair graph, or endpoint arithmetic is needed. Duplicates perform one update.

The pilot exposed a weakness in binary-search-only frontier processing: when
many different starts share one end, the frontier grows by one at every step
and performs unnecessary searches. The append variant first checks the last
tail. If it is at least `e`, the deepest chain can extend immediately. This also
makes DP linear for a full chain after sorting. Both frontier variants remain
in the benchmark, each with and without an explicit sortedness scan.

An offline CDQ variant was not implemented: the frontier already removes end
compression with a simpler invariant, and three independent required kernels
remain available for comparison.

#### Sorted-input decision

The production function always calls `sort_unstable_by`; it has no separate
sorted-input scan. Both Fenwick and frontier benchmarks tested an explicit
linear detection pass, with sorted, reverse, nearly sorted and shuffled input.
On this compiler the ordinary sort already handled fully sorted records just
as quickly as the scan. The added scan did not earn a separate production path.
This does not change the worst-case `O(n log n)` bound or promise a special
complexity contract for sorted callers. The raw samples retain both variants.

#### Native Polars investigation

We inspected the installed Polars expression API and the official documentation.
[Rolling rank](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.rolling_rank.html)
can count ordered predecessors, but does not compute their recursively defined
chain depths. [Fold](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.fold.html)
combines columns horizontally. Neither supplies this row-to-row ordered state.
[Cumulative evaluation](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.cumulative_eval.html)
evaluates expanding input windows and documents potentially quadratic work;
it does not directly expose the earlier outputs of this dynamic program.

We found no clean exact formulation with comparable asymptotic complexity as a
small composition of ordinary dataframe operations. This is an investigation
result, not a proof that no native formulation is possible. We do not benchmark
a pair-materializing self join followed by longest-path iteration or claim a
speedup over such a baseline. `containment_count` and nesting depth solve
different problems: several incomparable containers contribute several counts
but at most one step above their common child.

### Recorded core selections

The original full run explicitly selects
`A_packed_fenwick,A_sorted_fenwick,B_indirect_fenwick,C_packed_segment,D_frontier,D_sorted_frontier,D_append_frontier,D_sorted_append_frontier`
with five samples. The repeat selects sizes `1000000,3000000`, methods
`A_packed_fenwick,D_frontier,D_append_frontier,D_sorted_append_frontier,production`
and scenarios `chain,equal_ends,depth_4,dense,crossing,sorted,reverse,nearly,shuffled`.
The current default additionally includes the public production entry point.

### Phase and repeat evidence

Phase samples explain the larger differences. At 3M distinct-end rows, Fenwick
compression takes roughly 649–776 ms; the frontier omits it entirely. The
frontier then spends most time sorting: 129.71 ms sort and 49.80 ms DP/scatter
for a full chain, or 193.87 ms sort and 37.79 ms DP/scatter for dense random
input. Output scatter is fused into DP and recorded in that phase; output
allocation has its own column. Rank-buffer allocation belongs to compression
for tree candidates. These are independently computed phase medians, so they
need not sum exactly to the total median. Production repeat timings have no
internal phase clocks.

A second run records another **450 accepted samples**, including the public
production entry point without phase clocks, at 1M/3M rows on nine selected
families. At 3M, production medians are 214.70 ms (chain), 195.56 ms (equal
ends), 199.34 ms (depth 4), 252.11 ms (dense), 181.92 ms (crossing), 40.65 ms
(sorted), 189.30 ms (reverse), 166.75 ms (nearly sorted), and 253.78 ms
(shuffled). The corresponding packed Fenwick chain/depth-4/dense medians are
1301.84/1426.28/378.76 ms. The repeat supports the production choice while also
showing timing variation between runs. Explicit sorted detection is effectively
tied in the repeat's sorted case, 42.33 versus 42.44 ms, so the initial apparent
regression should be read as noise rather than a universal penalty.

### Validation details

Core fixtures include disjoint intervals, crossing antichains, full chains,
balanced binary nesting, all-identical rows, eight copies per chain geometry,
equal starts, equal ends, repeated empty coordinates, mixed empties/nonempties,
sparse and dense random containment, broad outer intervals, heavy overlap with
little containment, and independent depth-4/depth-64 families. Sorted, reverse,
nearly sorted and shuffled variants use the same dense geometry multiset.
The fixed generator seed is 42. Every one of the twenty families runs at 1K,
10K, 100K, 1M and 3M rows. Four 100K-row families also run with i32 Date physical
endpoints and with 10/100/1000 serial independent groups; the one-group case
is already present in the main matrix. Serial core grouping measures call-size
effects, while the plugin runner measures actual grouped Polars execution.

The core candidates are checked against an independent quadratic oracle before
timing small fixtures; full-size depth arrays must agree before samples are
accepted. All endpoint comparisons use the exact strict containment predicate.
The Python runner separately checks small fixtures against a quadratic oracle,
then checks every timed full output against analytic depth values carried through
the input permutation. Fixture construction, dtype conversion, expression
construction and output validation are excluded. Lazy optimization/physical
planning inside `collect`, collection and result Series retrieval are timed.

The plugin matrix covers Int64 at 1K, 10K, 100K, 1M and 3M rows, with crossing
antichains, depth 7 and 63 families, full chains and replicated depth-63 chains.
Each runs sorted, reverse, nearly sorted (a late inversion), and shuffled.
Date and Datetime(us) cover shallow and deep shuffled families at the same sizes.
At 100K total rows, 1/10/100/1000 groups cover antichains, shallow/deep chains and
duplicates across all three dtypes. Temporal casts are untimed; Date uses the
i32 kernel and Datetime uses i64, avoiding a redundant matrix of timestamp units.
Unit tests separately cover ms/us/ns and supported named timezones.

The production tests include named regressions for every tie and empty-endpoint
case, first-invalid-row validation, extremes of signed and unsigned integers,
original-order reconstruction, and a 100K-row chain. A memoized recursive
quadratic oracle directly follows the strict containment predicate without
sharing sorting, compression or tree logic. Exhaustive testing covers all
111,111 ordered collections of up to five rows over the ten valid geometries
whose endpoints lie in `[-1,2]`. For up to four rows, an independent subset
enumerator recognizes chains by pairwise strict comparability.

Proptest generates 0–25 intervals over `[-8,8]`, intentionally producing ties,
empties and crossings. The properties cover exact oracle agreement, length and
bounds, strict-container lower bounds, zero-depth characterization, permutation,
translation, positive scaling and reflection, duplicate insertion, own-depth
monotonicity under expansion/shrinking, constructed deep chains/antichains/
identical families/duplicate chains, strict-container count bounds, and global
maximum chain cardinality checked by subsets on tiny cases. A separate property
cross-checks `containment_count` against strict children plus identical rows.
Benchmark candidates have their own oracle properties, rank-compression checks,
duplicate batching regressions, and random Fenwick/segment-tree update/query
traces, including absence versus a present depth zero.
The production frontier has direct append/replacement/duplicate tests and a
property that recomputes the entire achievable-end frontier from an independent
quadratic DP after **every** processed geometry batch, not only at final output.

Historical package checks and commands are recorded in [nesting-package-checks-windows.json](results/nesting-package-checks-windows.json). The [pilot](results/nesting-pilot-windows.csv), [full core run](results/nesting-kernels-windows.csv), [production repeat](results/nesting-production-windows.csv), and [Polars run](results/nesting-polars-windows.csv) retain all measured variants and sample ranges.

## Lane assignment

### Candidates and optimality

All candidates validate lengths and interval direction, ignore empty intervals during
assignment, initialize their output to lane zero, and return `Vec<u32>` in original row
order. Sort ties use original row indices. Free lists are LIFO. The heap breaks end ties
by lane ID. None constructs an explicit graph.

| Candidate | Ordering and assignment | Time after sorting | Working space including output |
| --- | --- | --- | --- |
| A: heap | Sort non-empty indices by `(start, row)` and reuse the earliest-ending lane when `end <= start` | O(n log max(2, ω)) | O(n + ω) |
| B: two sorts | Sort non-empty indices separately by `(start, row)` and `(end, row)`. Release ended lanes before each start, then pop a free lane | O(n) | O(n + ω) |
| C: events | Sort `(endpoint, event kind, row)` with END before START, then release or pop lanes | O(n) | O(n + ω) |

Sorting is O(n log n) for every candidate. A uses `BinaryHeap<Reverse<...>>` and
replaces an available root using `peek_mut`, avoiding a separate pop/push. B needs no
heap. Any non-empty interval ending before the current start must already have been
assigned. C materializes two endpoint events per non-empty row. All sorting uses the
standard library's in-place unstable sort with explicit deterministic tie keys. No new
dependencies were added.

For A, if no lane can be reused, all existing lanes' latest intervals overlap the
current start. Those intervals plus the new one form a clique, so allocating another
lane is necessary. Reuse never puts overlapping intervals in one lane. Thus the lane
count equals maximum concurrency. The sweep candidates have the same invariant through
their active/free sets. Empties consume no capacity. A nonempty collection of only
empties uses lane zero, and empty input returns `[]`.

### Repeat evidence

A has the lowest median in 42of the 72 first-run cases, B in 24, C in 6. The repeat
gives A 45, B 24, C 3. The table below covers all sizes: geometric means of per-case
median ratios to A, giving each family/order equal weight. Values greater than one mean
slower than A. This weighting is descriptive, not an assumed distribution of user
workloads.

| Rows | B/A first | B/A repeat | C/A first | C/A repeat |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 1.119 | 1.093 | 2.897 | 2.885 |
| 10,000 | 1.044 | 1.057 | 2.332 | 2.399 |
| 100,000 | 1.044 | 1.030 | 2.679 | 2.638 |
| 1,000,000 | 0.918 | 0.979 | 2.078 | 2.257 |

The largest shuffled cases show noticeable run variation: disjoint A is 79.73 ms first
and 59.37 ms on repeat, while shuffled moderate A is 153.11 ms and 130.22 ms. We retain
both runs rather than selecting favorable samples. The main tradeoffs persist: on repeat
the sorted nested case is A/B/C 30.04/8.72/84.94 ms, and the shuffled nested case
167.18/109.94/95.23 ms.

At small concurrency, A avoids the second index sort and keeps a tiny heap. At larger
concurrency, heap work matters: the ordered nested case is about 3.3× faster with B, and
the shuffled nested case about 1.8× faster with C. C's larger event array makes it a
costly default for already ordered input.

### Workloads and validation

The harness is
[`crates/intervals-core/benches/assign_lanes.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/assign_lanes.rs),
with references and oracles in its `support` module. It refuses debug builds. Each
invocation tests 72 cases: nine families × two row orders × four sizes (1,000, 10,000,
100,000, 1,000,000). Endpoints are `i64`.

| Family | Construction for row i before shuffling | Maximum concurrency |
| --- | --- | --- |
| Disjoint | `[i, i+1)` | 1 |
| Low fixed concurrency | `[i, i+8)` | 8 |
| Moderate concurrency | `[i, i+128)` | 128 |
| Large clique | `[i, i+n)` | n |
| Nested | `[i, 2n-i)` | n |
| Long path / staircase | `[i, i+2)` | 2 |
| Ties | `s=16 floor(i/32)`, `[s, s+16+(i mod 4))` | 56 |
| Duplicates | `s=4 floor(i/16)`, `[s, s+8)` | 32 |
| Mixed empties | `s=floor(i/4)`, end `s` every third row, otherwise `s+8` | 22 |

Every family runs both already start-sorted and shuffled. A fixed xorshift64
Fisher–Yates shuffle, seeded with 42 at the start of the invocation, controls both input
and method order. All candidates receive exactly the same vectors.

Before timing a case, every candidate must pass an independent maximum-concurrency sweep
and a validity check. The concurrency oracle sorts signed endpoint deltas, processing
`-1` before `+1` at ties. The scalable validity check sorts non-empty rows within each
lane and rejects any overlapping neighbors. It also checks length and contiguous IDs.
Determinism is checked with a second call. The repeat run additionally checks production
against its measured heap reference. Small unit/proptest inputs also use an independent
O(n²) pairwise conflict oracle. Another coloring algorithm is never the optimality
oracle.

Each case has two warmups followed by nine samples per method, with shuffled method
order. Timers cover validation, allocation, sorting, assignment, and output
construction. Generation, correctness checks, and output destruction are outside timing.
Every timed output is verified again after stopping the timer. Failed checks abort the
run. Inputs and outputs pass through `black_box`.

See [first samples](results/assign-lanes-windows.csv), [repeat samples](results/assign-lanes-repeat-windows.csv) and [metadata](results/assign-lanes-environment.json).

## Weighted scheduling

### Candidates and recurrence

All candidates validate the original rows, omit nonpositive weights, and select positive
empty intervals separately. Empty intervals conflict with nothing. Their objective is
added to the ordinary schedule with checked `i128` arithmetic.

| Candidate | Preparation | Optimization and reconstruction |
| --- | --- | --- |
| A | Sort by `(end, start, original index)` and store sorted ends | Binary-search each compatible prefix, then run linear DP and backtracking |
| B | Use the same finish order. Sort finish positions independently by `(start, end, original index)` and sweep sorted ends to find prefixes | Linear DP and backtracking |
| C | Sort `(coordinate, event kind, original row)` events with ends before starts at equal coordinates | Snapshot the best objective and chain head at each start. Compare the completed candidate at its end, then reconstruct via saved heads |

For A/B, `OPT[j + 1] = max(OPT[j], weight[j] + OPT[p[j]])`, with `p` a prefix length.
Non-empty intervals guarantee `p[j] <= j`. A strict improvement selects the row. Ties
skip it. Original row indices complete every sort key, so identical input is
deterministic. These internal tie rules are not a stable public promise. C processes all
ends at a coordinate before starts, so touching intervals can chain. Separating empties
prevents start/end ordering from losing their weights.

Only B ships in the core. A/B/C references live in
`crates/intervals-core/benches/support/weighted.rs`, shared by tests and benchmarks. C
is retained there for reproduction and differential testing. Its larger event buffers
and generally slower measurements do not justify production complexity.

### Phase and storage details

For shuffled disjoint positive rows at 1M,coarse phase medians (milliseconds) show
where the time goes:

| Candidate | Preparation | Optimization | Reconstruction |
| --- | ---: | ---: | ---: |
| A | 50.08 | 94.26 | 3.00 |
| B | 65.63 | 11.66 | 3.05 |
| C | 88.78 | 68.88 | 39.61 |

### Timing and memory interpretation

The coarse phase timers use only three start/elapsed pairs per call. Preparation
includes validation, empty handling and sorting. B also includes its predecessor sweep
there. A performs binary searches inside the optimization phase. The DP or event sweep
includes its buffers and checked objective accumulation. Reconstruction measures only
following links/scanning DP entries and setting output bits. Total time also includes
buffer destruction, small timer/reporting costs, and function overhead. Medians of
phases need not add up to median total. Small measurements are especially sensitive to
timer and operating-system noise.

Memory is **peak simultaneously live vector capacity**, including the byte-per-row Rust
Boolean output, not process RSS. Allocation counts are derived from the nonempty
buffers, not an instrumented allocator. There are no vector growth reallocations or
allocating stable sorts. Inputs, stack frames, allocator metadata, and the independent
oracle are excluded. With `n` input rows and `m` positive non-empty rows on this 64-bit
target:

| Candidate | Peak buffer bytes | Nonempty-buffer allocations when `m > 0` |
| --- | ---: | ---: |
| A | `9n + 32m + 16` | 5 |
| B | `9n + 32m + 16` | 6 |
| C | `9n + 72m` | 5 |

B frees the extra `8m` start-order vector before allocating `16(m+1)` DP bytes, so it
adds an allocation without increasing peak live buffer capacity over A. At 1M positive
non-empty rows A/B use 41,000,016 bytes (39.10 MiB). C uses 81,000,000 bytes (77.25
MiB). The Polars adapter additionally materializes an `i128` weight vector (16 bytes per
row), borrows contiguous endpoint buffers, copies endpoints only for multiple chunks,
and builds Polars Boolean output. These adapter costs are outside this core-only
benchmark.

### Workloads and validation

The full matrix crosses 1K, 10K, 100K and 1M rows with both finish-sorted and shuffled
input, eleven structures, and six weight distributions: **528 workloads**. The
deterministic xorshift generator starts at seed 42. Two warmups and five recorded runs
per candidate shuffle candidate order, giving 7,920 samples per full run. Workload
construction and all correctness checks are outside timing.

| Structure | Intervals before sorting, for `i = 0..n-1` |
| --- | --- |
| Mostly disjoint | `[3i, 3i + 2)` |
| Low overlap | `[i, i + 8)` |
| Moderate overlap | `[i, i + 128)` |
| Dense overlap | `[i, i + n)` |
| Deep nesting | `[i, 2n - i)` |
| Long staircase | `[i, 2i + 2)` |
| Duplicates | `[4 floor(i/16), 4 floor(i/16) + 8)` |
| Equal starts/ends | `[16 floor(i/32), 16 floor(i/32) + 16 + (i mod 4))` |
| Touching | `[i, i + 1)` |
| Many empties | Start `floor(i/4)` with length zero for two thirds of rows and 8 otherwise |
| Random lengths | `[i, i + 1 + random(0..n/8))` with less-correlated start/finish orders |

The six weight distributions are:

- Uniformly positive, from 1 to 100.
- Mixed signs, from -100 to 100.
- About 80% zeros, with remaining values from 1 to 100.
- Small ties, from 1 to 3.
- Powers of two, from 1 through `2^62`.
- An expensive interval competing with many weight-10 rows.

In the last case row zero is replaced by an
interval spanning the whole ordinary input with weight `5n`. It wins in some dense cases
but loses to the combined disjoint schedule.

Before any timing is accepted, every candidate and production are checked against an
independent **start-sorted suffix DP**: at each row, skip it or take its weight plus the
optimum starting at the first start at least its end. It shares no candidate
preparation, finish ordering, predecessor links or reconstruction. Every timed mask is
checked for original-row length, selected positive empties, positive weights,
non-overlap, and that independently computed objective. Tied masks need not match.

Test-only exhaustive subset enumeration independently validates all candidates,
production, and the suffix oracle on small inputs. Proptest generates 0–12 rows, starts
in -4–4, lengths in 0–6 and signed weights -10–20 without filtering. Properties cover
feasibility, exact objective, shape, determinism, translation, row permutation,
negative-row additions, positive-empty additions, all-negative weights and positive
scaling. Additional deterministic tests cover greedy counterexamples, predecessor
chains, duplicates, integer boundaries, overflow, length validation and reversed
original row indices.

### Repeat and historical validation

Across all 528 workloads,B's geometric-mean runtime relative to A is **0.794**. C's is
**1.528**. Per-workload medians give 390 wins to B, 114 to A, and 24 to C. Each workload
has equal weight in this aggregate, independent of size/runtime. An independent repeat
with identical inputs gives **0.798** for B and **1.555** for C relative to A, with
389/119/20 wins for B/A/C. Both complete runs validate every output (15,840 recorded
candidate samples in total).


Validation at the recorded revision:

Completed locally at the recorded revision (Python 3.14.0; see the linked metadata):

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 65 tests and 6 doctests passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo doc --workspace --no-deps --locked`, `RUSTDOCFLAGS=-D warnings` | Passed |
| `uv lock --check` | Passed |
| `uv run --locked ruff check .` | Passed |
| `uv run --locked ruff format --check .` | Passed |
| `uv run --locked pytest --doctest-modules python/polars_intervals tests` | 485 passed |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed |
| CI/release helper tests and version/toolchain policy | 49 tests passed and policy passed |
| Release candidate benchmark | Two complete, verified 528-workload runs |
| Release wheel installed outside checkout, `python -I ...release_checks.py installed` | 485 passed with both Polars 1.44.2 and minimum Polars 1.44.1 |
| Wheel/source archive metadata and `twine check --strict` | Passed |

The wheel is a local CPython 3.14 Windows x86-64 release build. The installed artifact
checks confirm imports come from a separate temporary environment, exercise
integer/Date/Datetime smoke cases, then run the full tests and doctests. The source
archive was built and inspected for the implementation, tests, benchmark reference,
lockfile and compiler policy. Other OS/Python wheel targets remain covered by the
existing CI release matrix. They were not run locally.



See [first samples](results/weighted-windows.csv), [repeat samples](results/weighted-repeat-windows.csv) and [metadata](results/weighted-environment.json).

## Maximum k-coverage

### Sources and algorithm

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

### Adaptation and invariants

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

### Candidate implementations

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

### Fast paths

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

### Correctness gates

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


### Development comparisons

Full/unpruned tables are capped at four million `nk` cells, replay at thirty million `nk²` updates, and component convolution at 10K rows with `k<=64`. Omitted cases are safety skips, not zero-cost runs. The million-row matrix covers five representative families and budgets `0,1,8,64`.

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

#### Helper computation and record locality

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

#### Scaling in input size, budget and memory

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

#### Fast paths and component experiment

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

Detailed candidate phases, safety skips and validation counts remain in [core samples](results/coverage-core.csv) and [run metadata](results/coverage-environment.json); [collection samples](results/coverage-temporal.csv) have a separate timing scope. The measured source is preserved at commit `1abc545`; later readability changes named decision bits and separated reconstruction without changing the recurrence or candidate kernels.
