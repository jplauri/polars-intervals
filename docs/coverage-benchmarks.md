# Maximum k-coverage benchmarks

## Summary

[`max_k_coverage`](usage.md#select-maximum-coverage-with-a-budget) selects an exact
maximum-coverage subset within an interval budget. Production combines dominance
pruning, rolling dynamic-programming rows and exact fast paths; measured cost
rises with retained rows and budget, while reconstruction decisions require
memory proportional to their product.

## Results

**Polars collection · 24 threads · median of 3 samples ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json)**

--8<-- "docs/assets/benchmarks/coverage-collection-table.md"

The million-row staircase makes the budget cost visible. Date uses narrower
physical endpoints; these equivalent coordinate patterns do not establish a
universal dtype ranking. Collection includes planning, execution and Series
retrieval, with construction, casts and correctness checks outside timing.
No equivalent exact native Polars baseline was found or measured.

**Rust core · single-threaded · median of 3 samples · i64 ·
[same run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json)**

--8<-- "docs/assets/benchmarks/coverage-core-table.md"

Production has no internal phase clocks; references have them and use different
wrappers. It also specializes retained-record invariants and includes validation
and exact fast paths. It is therefore not simply an uninstrumented rolling-row
reference. Component decomposition wins the illustrated larger-budget component
case, but adds overhead at smaller budgets and on fully disjoint input; it is
not a production path. The saturation fixture shows the full-union fast path.

## Coverage and limitations

The core run covers 1,506 workloads across seventeen families, three orders,
1K–1M rows and budgets from zero through 64, with supplemental
empty and oversized-budget cases. The million-row subset uses five families and
budgets 0, 1, 8 and 64. Installed-plugin coverage is narrower: disjoint, staircase
and identical inputs, two orders, three dtypes and six budgets, without group
windows. Other temporal units and timezone behavior are correctness-tested only.

Full/unpruned tables are bounded at four million `nk` cells; replay at thirty
million `nk²` updates; component experiments at 10K rows and `k<=64`. Missing
candidate cells are safety omissions. Lower helper cost does not always lower
total runtime: the shuffled million-row staircase at `k=8` favored binary helper
lookup over the monotone-sweep reference in the recorded total. Packed records
also use more memory than indirect records; detailed comparisons remain in the
[development notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#development-comparisons).

**Peak requested live heap**, measured separately from timing, reaches 162 MiB
at `k=8` and 216 MiB at `k=64` for the million-row production staircase. At 10K
shuffled staircase rows and `k=32`, rolling rows use 2.07 MiB versus 21.0 MiB for
full tables; replay saves memory but can substantially increase runtime. Polars
memory is unmeasured. See shared [metric definitions](benchmarking.md).

Small cases use exhaustive subset oracles; a separate quadratic DP checks larger
test cases. Full benchmark masks are independently merged to verify objective
and count against the reference result. Collection fixtures have known optima.
The saved timings precede a readability refactor after measured commit `1abc545`;
the recurrence, memory layout and candidate kernels were unchanged.

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench max_k_coverage --locked > target/coverage-core.csv
# From an external environment with the release wheel installed:
python -I /path/to/checkout/benchmarks/coverage_temporal.py > coverage-temporal.csv
```

The core harness performs separate correctness and allocation calls before
three timed invocations. See the shared guide for release setup.

</details>

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-core.csv) ·
[Collection samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv) ·
[Metadata, source hashes and validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json) ·
[Recurrence, proofs and experiments](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#maximum-k-coverage)
