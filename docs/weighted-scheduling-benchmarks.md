# Weighted scheduling benchmarks

## Summary

[`max_weight_non_overlapping`](usage.md#select-a-globally-maximum-weight-schedule)
selects a maximum-weight schedule. Production uses two sorted orders and a linear
predecessor sweep before dynamic programming: it improves the measured structured
workloads, but loses on random lengths where start and finish orders are less
correlated, while retaining the binary-search candidate's peak buffer capacity.

## Results

**Rust core only · single-threaded · median of 5 samples · i64 endpoints and
weights · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json)**

All rows below use positive weights. These are candidate timings, including
coarse phase clocks; “production design” identifies the selected algorithm,
not a separate uninstrumented call to the public entry point.

--8<-- "docs/assets/benchmarks/weighted-table.md"

The linear sweep removes substantial predecessor-search work on structured
families. Random lengths make its second sort costly: the binary-search method
wins the finish-sorted example, while the endpoint sweep wins the shuffled
example. Some expensive-interval cases also favor binary search. A workload mix
dominated by independently ordered starts and finishes would justify reconsidering
the default.

Across all 528 equally weighted workload cells, the production candidate's
geometric mean runtime relative to binary search is 0.794 in the first run and
0.798 in an independent repeat. This is a description of the synthetic matrix,
not an assumed user-workload distribution. Both runs preserve the random-length
counterexamples; [repeat evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#repeat-and-historical-validation)
and raw sample ranges remain available.

## Coverage and limitations

The matrix crosses 1K–1M rows, eleven structures, finish-sorted/shuffled input
and six weight distributions. It includes disjoint and dense overlap, nesting,
duplicates, endpoint ties, empties, random lengths, mixed signs, mostly zeros,
small tied weights, wide powers of two and one expensive competing interval.
Two warmups precede five timed samples, with shuffled candidate order.

Timing includes input validation, sorting, predecessor computation, optimization,
reconstruction, temporary-buffer destruction and phase-clock overhead. The input
fixtures and independent checks are untimed. Only i64 endpoints and weights were
measured; objectives use checked i128 arithmetic. There are no end-to-end Polars,
temporal-endpoint or native Polars baseline timings. The adapter's i128 weight
buffer and Boolean output construction are outside this scope.

At 1M positive non-empty rows, **peak live buffer capacity** is 39.1 MiB for
both two sorted orders and binary search, versus 77.2 MiB for the endpoint sweep.
Production's extra start-order allocation is freed before the larger DP buffer
is allocated, so it increases allocation count without increasing peak capacity.
These are buffer capacities, not allocator-instrumented heap measurements or
process RSS; the [shared guide](benchmarking.md) defines the metrics.

Every timed mask is checked for feasibility and objective against an independent
start-sorted suffix DP, which shares neither finish ordering nor candidate
predecessor links. Small tests additionally use exhaustive subset enumeration,
including validation of that suffix oracle. Masks may differ under equal-objective
ties. Detailed recurrence, phase accounting and historical validation are in the
[design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling).

## Reproduce and data

<details markdown="1" id="historical-validation">
<summary>Operation-specific command and historical validation</summary>

```sh
cargo bench -p intervals-core --bench max_weight_non_overlapping --locked > target/weighted-local.csv
```

Set `WEIGHTED_BENCH_MAX_N=1000` for a smoke run. The core harness requires release
mode and does not need Polars. Historical package-check results moved to the
[validation record](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#repeat-and-historical-validation).

</details>

[Shared setup and publishing](benchmarking.md) ·
[First-run samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv) ·
[Repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-repeat-windows.csv) ·
[Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json) ·
[Algorithm notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling)
