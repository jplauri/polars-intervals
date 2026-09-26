The following workload and correctness coverage belongs to the shared minimum-cover and minimum-cost-cover harness.

`COVER_BENCH_MAX` and `COVER_BENCH_SAMPLES` optionally limit size and repeats.
Defaults are one million rows and three timed samples. The candidate harness
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
additional unit-cost control. The expensive-long family supplies the actual
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
implementations cross-check larger instances. The exponential oracle is bounded
to small correctness instances, and the quadratic candidate to 1K benchmark rows.
The native suite checks coverage on physical coordinates and deterministic masks.

### Independent correctness and integration

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
strictly advance. Unreachable states stay unreachable. Regressions cover equal
right ends, both greedy traps, duplicate costs, and exact `i128` overflow behavior.

Rust Polars and native Python tests exercise all supported endpoint/cost integer
widths, Date, all Datetime units and supported timezone metadata, exact target
conversion, eager/lazy/filter/window/group aggregation, streaming, multiple
chunks, original row order, invalid intervals, nulls and unsupported cost types.
Python also runs a second seeded subset oracle through the compiled plugin.
