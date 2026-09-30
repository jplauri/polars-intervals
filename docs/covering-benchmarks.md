# Minimum covering benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cover`](api.md#polars_intervals.minimum_cover) selects the fewest
intervals needed to cover a target range without gaps. Full Polars queries took
**51–69 ms for one million shuffled integer intervals** in the two synthetic
examples below. The selection is exact. Polars has no built-in solver for this
optimization problem. Overlap patterns and endpoint types affect runtime.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/cover-summary.md:3:-3"

Both million-row examples finish in well under a tenth of a second. The input
with many overlapping intervals is faster than the chain in this run.

Measurements used a September 26, 2026 build labeled 0.1.0. Input checks have
since changed. The updated full Polars query has not been timed.

<details markdown="1">
<summary>Benchmark details</summary>

<span id="operation-notes"></span>

<span id="full-polars-measurements"></span>

**What was compared**

The headline table compares the two measured shuffled integer workloads at one
million rows: a continuous chain and many overlaps.

--8<-- "docs/assets/benchmarks/cover-summary.md:-2:"

Use [minimum-cost covering](cost-covering-benchmarks.md) when intervals have different costs.

The first input forms a continuous chain of intervals whose endpoints touch.
The second has many overlapping intervals. Both are shuffled. Columns compare
integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/cover-polars-table.md"

Date endpoints are faster here. Their smaller stored representation can reduce
sorting work, but these measurements do not isolate the cost of handling types.

**Settings**

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

**Complete Polars queries · one solver thread · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

Each successful result is checked for complete target coverage. Algorithm
implementations are also compared for the number of selected intervals.
Small tests try every subset to verify optimality. Polars tests check coverage
and repeatable selections, without independently proving optimality at full size.

Query times include function calls, validation, selection and output construction.
Creating inputs, queries and type conversions is excluded. The
[validation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering-methodology.md)
retain the exact checks and measurement boundaries.

<span id="coverage-and-limitations"></span>

**Limitations**

The shared covering tests span 1,000 to one million rows and seventeen input
patterns. They include sorted, nearly sorted and shuffled data, duplicates,
empty intervals, rows outside the target, gaps and targets that cannot be
covered. Polars tests cover eight integer and temporal endpoint types on a
smaller selection of patterns. No Polars-only alternative was benchmarked.

The separate algorithm and Polars datasets do not support subtracting their
times to estimate overhead.

<span id="reproduce-and-data"></span>

<span id="operation-specific-commands"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
python -I /path/to/checkout/benchmarks/covering_temporal.py > covering-temporal-local.csv
```

The Rust target measures both covering operations. `COVER_BENCH_MAX` and
`COVER_BENCH_SAMPLES` restrict it. Run the final command with the installed release
wheel's Python from outside the checkout, following the shared setup guide.

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv) ·
[Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#covering) ·
[Validation and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)

<span id="underlying-algorithm-and-memory-comparisons"></span>

<span id="historical-validation"></span>

**History**

The saved measurement record is dated `2026-09-26T15:21:30.752129+00:00`, with
base revision `cee8e52e96a9a260af712d84fb86574c6225d309`. Artifact hashes identify
an explicitly recorded `polars_intervals-0.1.0-cp314-cp314-win_amd64.whl`.
The measured core source hash matches the covering solver in `1f27171`.
Metadata records the later adapter cleanup separately, including common plugin
input validation and fewer scalar copies. The record does not isolate the
runtime effect of those adapter changes. Harness verification and CSV summary
cleanup are outside the timed query, and no cleanup speedup is claimed.

**Rust algorithm only · one thread · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

The package sorts interval records together. An alternative sorts row indices,
and another keeps candidate intervals in a heap. All seek the same minimum
number of intervals. These shuffled inputs differ from the Polars examples,
and the timings include internal measurement overhead. “32-bit endpoints” uses
the storage width of Date values without calling Polars.

--8<-- "docs/assets/benchmarks/cover-core-table.md"

Sorting records wins on the chain and dense-overlap examples, but sorting
indices wins when every interval has the same start. On the million-row integer
chain, the package requests 26.2 MB of live heap storage compared with 9.39 MB
for the index-sorting alternative. This includes output and working buffers,
not total process memory.

</details>
