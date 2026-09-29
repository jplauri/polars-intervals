# Containment counting benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`containment_count`](api.md#polars_intervals.containment_count) counts how many
other intervals each row contains. Complete Polars queries on **three million
integer intervals took 65 ms to 1.09 seconds** in the displayed examples,
depending on the data. The Polars-only comparison took 146 ms to 2.01 seconds.
Many small groups can reverse that advantage. These measurements use an earlier
package build, before a change that reduced input copying.

## Results

**Complete Polars queries · 24 Polars threads · median of 5 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json)**

The comparison uses Polars ranking expressions to count containment without
building a row for every matching pair. All endpoints are Int64. “No grouping”
means that every interval is compared with the full input. Grouped row counts
are totals across all groups.

--8<-- "docs/assets/benchmarks/containment-polars-table.md"

Repeated intervals are much cheaper than the more irregular inputs in this run.
The grouped example has only ten rows per group on average. Here, the overhead
of calling the plugin outweighs its counting advantage. A separate earlier
join-based comparison also favored Polars on small groups.

<details markdown="1">
<summary>Underlying algorithm comparisons</summary>

**Rust algorithm only · one thread · median of 5 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.json)**

These compare data structures used for counting. The package uses the packed
Fenwick design, which stores sortable records together. Indirect Fenwick sorts
row indices instead. The segment tree is another counting structure. These
timings include internal measurement overhead and exclude Polars.

--8<-- "docs/assets/benchmarks/containment-core-table.md"

The selected design is faster on the large irregular examples, but loses on
duplicates. At three million distinct ends its live buffer capacity reaches
137 MiB, compared with 91.6 MiB for indirect Fenwick. This counts algorithm
buffers, including output, rather than the memory of the whole process.

</details>

## Coverage and limitations

Algorithm tests cover fourteen input patterns with 1,000 to three million rows,
including nesting, duplicates, shared endpoints, empty intervals and different
input orders. Polars tests also cover groups, Date and timezone-aware Datetime.
Small cases are checked by comparing every pair directly. Larger outputs are
cross-checked between implementations.

Query times include preparing the data for counting and creating the output.
The plugin validates inputs, while the Polars comparison assumes valid endpoints.
The current package avoids some copies made by the measured build, but has not
been remeasured here. The Polars ranking APIs were marked unstable at that time.

Some join-based alternatives were skipped when they could create more than two
million intermediate rows. The ranking comparison above avoids that expansion.
Algorithm-only and Polars tests use different datasets, so their times cannot
be subtracted to calculate Polars overhead. Detailed checks and memory accounting
remain in the [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md).

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench containment --locked
uv run --locked --no-sync python benchmarks/containment_count.py --methods plugin rank_runs --output target/containment-rank-runs.json
```

Use the shared release setup first. To reproduce the original native selection,
use `--methods plugin rank rank_by join bands equality` and a separate output.
`CONTAINMENT_SIZES` and `CONTAINMENT_CSV` select algorithm-test sizes and output.
The Python runner accepts `--sizes` and `--repeats`.

</details>

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv) ·
[Original native samples, skips, plans and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-windows.json) ·
[Run-rank samples and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json) ·
[Design notes and historical comparisons](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md)
