# Minimum covering benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cover`](api.md#polars_intervals.minimum_cover) selects the fewest
intervals needed to cover a target range without gaps. Complete Polars queries
took **51–69 ms for one million shuffled integer intervals** in the two displayed
examples. The selection is exact. Overlap patterns and endpoint types affect
runtime, and the algorithm trades additional working memory for speed on varied
inputs. Use [minimum-cost covering](cost-covering-benchmarks.md) when intervals
have different costs.

## Results

**Complete Polars queries · one solver thread · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

The first input forms a continuous chain of intervals whose endpoints touch.
The second has many overlapping intervals. Both are shuffled. Columns compare
integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/cover-polars-table.md"

Both examples finish in well under a tenth of a second at one million rows.
Date endpoints are faster here. Their smaller stored representation can reduce
sorting work, but these measurements do not isolate the cost of handling types.

<details markdown="1">
<summary>Underlying algorithm and memory comparisons</summary>

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

## Coverage and limitations

The shared covering tests span 1,000 to one million rows and seventeen input
patterns. They include sorted, nearly sorted and shuffled data, duplicates,
empty intervals, rows outside the target, gaps and targets that cannot be
covered. Polars tests cover eight integer and temporal endpoint types on a
smaller selection of patterns. No Polars-only alternative was benchmarked.

Each successful result is checked for complete target coverage. Algorithm
implementations are also compared for the number of selected intervals.
Small tests try every subset to verify optimality. Polars tests check coverage
and repeatable selections, without independently proving optimality at full size.

Query times include plugin calls, validation, selection and output construction.
Creating inputs, queries and type conversions is excluded. The measured package
predates a cleanup of input handling, with the selection algorithm unchanged.
The separate algorithm and Polars datasets do not support subtracting their times
to estimate overhead. The [validation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering-methodology.md)
retain the exact checks and measurement boundaries.

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_summary.py benchmarks/results/covering-local.csv
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
