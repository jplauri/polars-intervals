# Containment counting benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`containment_count`](api.md#polars_intervals.containment_count) counts how many
other intervals each row contains. In synthetic benchmarks, full Polars queries
processed **three million repeated intervals in 65.3 ms**, **2.24× faster** than
the native Polars ranking expressions tested. With few containments, the same
size took **1.09 seconds**, **1.85× faster**. Many small groups reverse the
advantage. These measurements use an earlier package build.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/containment-summary.md:3:-3"

Repeated intervals are much cheaper to count than irregular inputs. With only
ten rows per group on average, calling the plugin costs more than it saves.

The current version has not been remeasured.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and native comparison**

The headline cases contrast large repeated and irregular inputs with a small
grouped input where native Polars is faster.

The Polars run used 24 Polars threads, one warmup and five samples per case,
with seed 42. The displayed times are medians. Settings, software versions and
source hashes are recorded in the
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json).

The comparison uses Polars ranking expressions to count containment without
building a row for every matching pair. All endpoints are Int64. “No grouping”
means that every interval is compared with the full input. Grouped row counts
are totals across all groups.

Ranking expressions were the only native method measured in this run. Other
native implementations were measured separately in the original selection run,
whose timings are not mixed into this comparison. Speedup is native median
divided by package median. The loss uses package median divided by native median.

--8<-- "docs/assets/benchmarks/containment-summary.md:-2:"

--8<-- "docs/assets/benchmarks/containment-polars-table.md"

Repeated intervals are much cheaper than the more irregular inputs in this run.
The grouped example has only ten rows per group on average. Here, the overhead
of calling the plugin outweighs its counting advantage. A separate earlier
join-based comparison also favored Polars on small groups.

**Underlying algorithm comparisons**

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

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

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

<span id="reproduce-and-data"></span>

**Reproduce and data**

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

</details>
