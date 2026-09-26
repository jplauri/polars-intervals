# Choosing the lane assignment algorithm

The production default is **A: start sort + min-heap**. It balances runtime
across input sizes and orders with low working memory at small concurrency.
It is not universally fastest: two sorted streams win many ordered workloads,
and an endpoint-event sweep wins some large shuffled workloads. Only A ships
in the library. B and C remain private benchmark/test references.

## Exact candidates

All candidates validate lengths and interval direction, ignore empty intervals
during assignment, initialize their output to lane zero, and return `Vec<u32>`
in original row order. Sort ties use original row indices. Free lists are LIFO;
the heap breaks end ties by lane ID. None constructs an explicit graph.

| Candidate | Ordering and assignment | Time after sorting | Working space including output |
| --- | --- | --- | --- |
| A: heap | Sort non-empty indices by `(start, row)`; reuse the earliest-ending lane when `end <= start` | O(n log max(2, ω)) | O(n + ω) |
| B: two sorts | Independently sort non-empty indices by `(start, row)` and `(end, row)`; release ended lanes before each start; pop a free lane | O(n) | O(n + ω) |
| C: events | Sort `(endpoint, event kind, row)` with END before START; release/pop lanes | O(n) | O(n + ω) |

Sorting is O(n log n) for every candidate. A uses `BinaryHeap<Reverse<...>>`
and replaces an available root using `peek_mut`, avoiding a separate pop/push.
B needs no heap; any non-empty interval ending before the current start must
already have been assigned. C materializes two endpoint events per non-empty row.
All sorting uses the standard library's in-place unstable sort with explicit
deterministic tie keys; no new dependencies were added.

For A, if no lane can be reused, all existing lanes' latest intervals overlap
the current start. Those intervals plus the new one form a clique, so allocating
another lane is necessary. Reuse never puts overlapping intervals in one lane.
Thus the lane count equals maximum concurrency. The sweep candidates have the
same invariant through their active/free sets. Empties consume no capacity; a
nonempty collection of only empties uses lane zero, and empty input returns `[]`.

## Reproduction and methodology

```sh
cargo bench -p intervals-core --bench assign_lanes --locked > benchmarks/results/assign-lanes-local.csv
```

The harness is
[`crates/intervals-core/benches/assign_lanes.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/assign_lanes.rs),
with references and oracles in its `support` module. It refuses debug builds.
Each invocation tests 72 cases: nine families × two row orders × four sizes
(1,000, 10,000, 100,000, 1,000,000). Endpoints are `i64`.

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
Fisher–Yates shuffle, seeded with 42 at the start of the invocation, controls
both input and method order. All candidates receive exactly the same vectors.

Before timing a case, every candidate must pass an independent maximum-concurrency
sweep and a validity check. The concurrency oracle sorts signed endpoint deltas,
processing `-1` before `+1` at ties. The scalable validity check sorts non-empty
rows within each lane and rejects any overlapping neighbors. It also checks
length and contiguous IDs. Determinism is checked with a second call. The
repeat run additionally checks production against its measured heap reference.
Small unit/proptest inputs also use an independent O(n²) pairwise conflict oracle.
Another coloring algorithm is never the optimality oracle.

Each case has two warmups followed by nine samples per method, with shuffled
method order. Timers cover validation, allocation, sorting, assignment, and output
construction. Generation, correctness checks, and output destruction are outside
timing. Every timed output is verified again after stopping the timer; failed
checks abort the run. Inputs and outputs pass through `black_box`.

Measurements were made on 2026-09-26, Windows 11 Home 10.0.26200, AMD Ryzen 9
3900X (12 cores/24 logical processors), Rust 1.98.1, x86_64-pc-windows-msvc,
default optimized Cargo bench profile. The algorithms are single-threaded.
Two complete invocations are retained, each with 1,944 accepted samples:
[first run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv),
[repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-repeat-windows.csv).
[Environment metadata and algorithm source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json)
are retained alongside them. The branch builds on PR 20 head `b644871`.

## Results and decision

First-run medians at one million rows, milliseconds (lower is better):

| Family | Order | A: heap | B: two sorts | C: events |
| --- | --- | ---: | ---: | ---: |
| Disjoint | sorted | 5.78 | 10.28 | 18.66 |
| Disjoint | shuffled | 79.73 | 137.07 | 101.40 |
| Low, ω=8 | sorted | 12.60 | 10.27 | 80.76 |
| Low, ω=8 | shuffled | 116.73 | 142.29 | 99.95 |
| Moderate, ω=128 | sorted | 23.52 | 10.20 | 87.93 |
| Moderate, ω=128 | shuffled | 153.11 | 140.08 | 100.11 |
| Clique | sorted | 13.61 | 8.79 | 83.90 |
| Clique | shuffled | 91.85 | 113.58 | 96.18 |
| Nested | sorted | 29.93 | 9.08 | 85.60 |
| Nested | shuffled | 171.93 | 121.38 | 95.98 |
| Staircase | sorted | 10.07 | 10.57 | 80.09 |
| Staircase | shuffled | 89.21 | 137.37 | 101.01 |
| Ties | sorted | 21.66 | 39.05 | 102.02 |
| Ties | shuffled | 152.50 | 142.48 | 126.52 |
| Duplicates | sorted | 17.67 | 10.19 | 96.71 |
| Duplicates | shuffled | 142.91 | 140.06 | 136.15 |
| Mixed empties | sorted | 12.47 | 7.67 | 64.89 |
| Mixed empties | shuffled | 81.79 | 89.86 | 78.82 |

A has the lowest median in 42 of the 72 first-run cases, B in 24, C in 6.
The repeat gives A 45, B 24, C 3. The table below covers all sizes: geometric
means of per-case median ratios to A, giving each family/order equal weight.
Values greater than one mean slower than A; this weighting is descriptive,
not an assumed distribution of user workloads.

| Rows | B/A first | B/A repeat | C/A first | C/A repeat |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 1.119 | 1.093 | 2.897 | 2.885 |
| 10,000 | 1.044 | 1.057 | 2.332 | 2.399 |
| 100,000 | 1.044 | 1.030 | 2.679 | 2.638 |
| 1,000,000 | 0.918 | 0.979 | 2.078 | 2.257 |

The largest shuffled cases show noticeable run variation: disjoint A is
79.73 ms first and 59.37 ms on repeat, while shuffled moderate A is 153.11 ms
and 130.22 ms. We retain both runs rather than selecting favorable samples.
The main tradeoffs persist: on repeat the sorted nested case is A/B/C
30.04/8.72/84.94 ms, and the shuffled nested case 167.18/109.94/95.23 ms.

At small concurrency, A avoids the second index sort and keeps a tiny heap.
At larger concurrency, heap work matters: the ordered nested case is about
3.3× faster with B, and the shuffled nested case about 1.8× faster with C.
C's larger event array makes it a costly default for already ordered input.

## Memory and limits

The CSV records peak live **buffer capacity bytes**, measured from the actual
Vec/BinaryHeap capacities and element sizes. This includes the output, excludes
the two borrowed input slices, allocator metadata, stack storage, and transient
reallocation peaks. It is not RSS or a total process-memory measurement. There
is no allocator instrumentation in the timed loop, and sort allocates no buffer.

At one million non-empty rows on this 64-bit target:

- A reserves about 12.00 MB at low concurrency (8 MB indices + 4 MB output +
  a small heap). At a million-way clique its grown heap brings this to 28.78 MB.
- B reserves about 20.00 MB (two index arrays + output + a small free list);
  at the clique no lane is released during the start sweep, so the free list
  remains empty. It therefore uses less memory than A on that case.
- C reserves about 52.00 MB at low concurrency and 56.19 MB for the clique.
  Each `(i64, bool, usize)` event occupies 24 bytes including padding. Its final
  END events release all lanes, so the free list eventually holds the clique.

A's smaller working set on ordinary concurrency and its broad runtime results
justify the simple heap default. B and C are useful exact references and future
comparison points, not public options or production branches. We do not infer
an automatic crossover threshold from this synthetic matrix.

These are core-algorithm measurements, not Python end-to-end benchmarks. They
do not establish temporal-specific speedups, results on other hardware, or a
universal winner. Synthetic families cover important extremes but do not model
every real workload. Close timings and small-run differences should not be
overinterpreted. Public lane-ID semantics deliberately permit future production
changes while preserving optimality, contiguity, determinism, and original order.
