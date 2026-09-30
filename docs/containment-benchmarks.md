# Containment counting benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`containment_count`](api.md#polars_intervals.containment_count) counts how many
other intervals each row contains. In synthetic benchmarks, full Polars queries
processed **three million repeated intervals in 65.3 ms**, **2.24× faster** than
the native Polars ranking expressions tested. With few containments, the same
size took **1.09 seconds**, **1.85× faster**. Many small groups reverse the
advantage.

## Results

**Full Polars query time · milliseconds**

Native Polars uses ranking expressions to count the same containments without
creating a row for every matching pair.

--8<-- "docs/assets/benchmarks/containment-summary.md:3:-3"

Repeated intervals are much cheaper to count than irregular inputs. With only
ten rows per group on average, calling the function costs more than it saves.

Measurements were taken on September 26, 2026 with a build that copied
endpoint columns. The current function avoids some of these copies and has not
been remeasured.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The headline cases contrast large repeated and irregular inputs with a small
grouped input where native Polars is faster.

The comparison uses Polars ranking expressions to count containment without
building a row for every matching pair. Ranking expressions were the only native
method measured in this run. Other native implementations were measured
separately in the original selection run, whose timings are not mixed into this
comparison.

Speedup is native median divided by package median. Above 1 means the package
is faster. Below 1 means it is slower.

--8<-- "docs/assets/benchmarks/containment-summary.md:-2:"

--8<-- "docs/assets/benchmarks/containment-polars-table.md"

**Settings**

The Polars run used 24 Polars threads, one warmup and five samples per case,
with seed 42. The displayed times are medians. All endpoints are Int64.
“No grouping” means that every interval is compared with the full input.
Grouped row counts are totals across all groups.

Query times include preparing the data for counting and creating the output.
See the shared [hardware](benchmarks.md#hardware). Settings, software versions
and source hashes are recorded in the
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json).

<span id="coverage-and-limitations"></span>

**Limitations**

Algorithm tests cover fourteen input patterns with 1,000 to three million rows,
including nesting, duplicates, shared endpoints, empty intervals and different
input orders. Polars tests also cover groups, Date and timezone-aware Datetime.
Small cases are checked by comparing every pair directly. Larger outputs are
cross-checked between implementations.

The function validates inputs, while the Polars comparison assumes valid
endpoints. The Polars ranking APIs were marked unstable at that time.

Some join-based alternatives were skipped when they could create more than two
million intermediate rows. The ranking comparison above avoids that expansion.
Algorithm-only and Polars tests use different datasets, so their times cannot
be subtracted to calculate Polars overhead. Detailed checks and memory accounting
remain in the [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md).

<span id="reproduce-and-data"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench containment --locked
uv run --locked --no-sync python benchmarks/containment_count.py --methods plugin rank_runs --output target/containment-rank-runs.json
```

Use the shared release setup first. To reproduce the original native selection,
use `--methods plugin rank rank_by join bands equality` and a separate output.
`CONTAINMENT_SIZES` and `CONTAINMENT_CSV` select algorithm-test sizes and output.
The Python runner accepts `--sizes` and `--repeats`.

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv) ·
[Original native samples, skips, plans and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-windows.json) ·
[Run-rank samples and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json) ·
[Design notes and historical comparisons](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md)

**History**

The headline ranking follow-up was recorded at
`2026-09-26T19:50:36.278107+00:00`, with dirty base revision
`925033f9990b47f473d9debb2779b4533e63f24f`. The measured wheel's package version
and adapter source hash were not recorded, so the base revision's `0.1.0`
manifest does not establish the installed wheel's identity. The run retains
kernel and runner hashes. Its historical adapter copied both endpoint columns.
The current shared extraction borrows contiguous columns and collects only
when necessary. That extraction is inside the complete-query timing. Moving
candidate files and consolidating provenance are outside the timed operation.

The earlier join-based comparison also favored Polars on small groups. Rust-only
tests compared packed Fenwick, indirect Fenwick and segment-tree designs. The
package uses packed Fenwick, which stores sortable records together. Indirect
Fenwick sorts row indices instead. These timings include internal measurement
overhead and exclude Polars: one thread, median of five samples, with
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.json).

--8<-- "docs/assets/benchmarks/containment-core-table.md"

The selected design is faster on large irregular examples, but loses on
duplicates. At three million distinct ends its live buffer capacity reaches
137 MiB, compared with 91.6 MiB for indirect Fenwick. This counts algorithm
buffers, including output, rather than the memory of the whole process.

</details>
