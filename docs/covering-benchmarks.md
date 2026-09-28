# Minimum covering benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cover`](api.md#polars_intervals.minimum_cover) selects the fewest
intervals covering one target. Production's packed greedy sweep outperforms
indirect sorting on varied shuffled inputs, but loses on some repeated geometry
and uses more memory. The weighted operation has a separate
[minimum-cost covering report](cost-covering-benchmarks.md).

## Results

**Polars collection · single-threaded solver, Polars pool size unrecorded · median
of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

--8<-- "docs/assets/benchmarks/cover-polars-table.md"

These installed release-wheel calls time eager selection and Series retrieval,
including plugin dispatch, validation, physical adaptation and mask construction.
Input generation, expression construction and casts are excluded. Narrower Date
records can reduce sorting cost, but the measurements do not isolate dtype
adaptation. The separate core fixtures differ, so subtracting core runtime from
collection time would not estimate adapter overhead.

**Rust core · single-threaded · median of 3 samples · internal phase clocks included
· [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

Shuffled inputs; the recorded random costs identify the shared fixture but do
not affect minimum-cardinality selection.

--8<-- "docs/assets/benchmarks/cover-core-table.md"

Packed and indirect greedy methods differ in sorting records versus original-row
indices. Heap is a separate greedy implementation. Repeated starts favor indirect
sorting; mostly irrelevant rows spend little time selecting and do not establish
a reliable winner from these small differences. Packed storage favors varied
shuffled geometry at a material memory cost: the 1M Int64 chain uses 26.2 MB of
peak **requested live heap**, versus 9.39 MB for indirect greedy.

Production has no explicit sortedness-detection pass. That candidate's benefits
were inconsistent: it changed the 1M sorted-chain median from 13.9 to 14.1 ms,
and the sorted Datetime-width dense case from 14.3 to 15.3 ms. Its full results
remain in the shared raw samples; the main table avoids an almost identical
extra production series.

## Coverage and limitations

The shared covering suite measures 1K–1M rows across 17 geometry/difficulty
families, sorted, nearly sorted and shuffled order, plus Date/Datetime physical
widths. It includes failed covers, gaps, empty and irrelevant rows, duplicates
and repeated endpoints. The cost-distribution dimension belongs to the shared
weighted harness; this is a bounded set of slices, not a complete Cartesian
matrix. Actual release-wheel coverage includes eight integer/temporal dtypes
on chains, dense overlap, duplicates and irrelevant inputs.

Before accepting core timings, each successful mask must independently cover
the target and each candidate's objective must match production. Exhaustive
subset optimality checks are restricted to small correctness cases. The temporal
suite checks coverage and deterministic masks; it is not an independent
full-size optimum oracle. The [shared correctness details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering-methodology.md)
keep that distinction explicit.

There is no native Polars expression baseline. Core totals include validation,
clipping, sorting, selection and temporary-buffer destruction, excluding output
destruction. Allocation data comes from a separate untimed call. The recorded
wheel predates adapter cleanup; the measured solvers are unchanged, and metadata
keeps the original and cleanup source hashes separate.

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
python -I /path/to/checkout/benchmarks/covering_temporal.py > covering-temporal-local.csv
```

The Rust target measures both covering operations. `COVER_BENCH_MAX` and
`COVER_BENCH_SAMPLES` restrict it. Run the final command with the installed release
wheel's Python from outside the checkout, following the shared setup guide.

</details>

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv) ·
[Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#covering) ·
[Validation and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)
