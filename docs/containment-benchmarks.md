# Containment counting benchmarks

## Summary

[`containment_count`](usage.md#count-containment) counts contained rows with a
packed Fenwick tree. The recorded plugin outperforms native run rank on the
large global examples below, but native expressions win with many tiny groups;
the packed layout also trades memory and ordered-input performance for faster
irregular core workloads.

## Results

**Polars collection · 24 threads · median of 5 samples ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json)**

**Historical adapter:** these samples include unconditional endpoint copies.
The current shared adapter borrows contiguous columns; it has not been remeasured
here. Native run rank is an exact, non-pair-materializing Polars alternative.

--8<-- "docs/assets/benchmarks/containment-polars-table.md"

Each row compares methods from the same follow-up run. Native preprocessing,
row-order restoration and output materialization are timed; the plugin additionally
validates public inputs, while native queries assume valid matching endpoints.
Tiny groups can reverse the result. In the separate original run, the equality
join took 3.63 ms against the plugin's 7.75 ms on 10K sparse rows in 1,000 groups.

**Rust core · single-threaded · median of 5 samples · i64 ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.json)**

--8<-- "docs/assets/benchmarks/containment-core-table.md"

These are instrumented candidate implementations: totals include validation,
compression, sorting, allocation, counting and phase clocks. Packed Fenwick is
the production design, but it does not win every family or the equally weighted
all-size comparison. Its lower absolute costs on larger irregular workloads
motivated the choice; indirect Fenwick saves copying and memory.

## Coverage and limitations

Core coverage crosses fourteen structures with 1K–3M rows: containment density,
duplicates, equal endpoints, empties and input order. Polars additionally covers
10K/100K total rows in 1–1,000 group windows, Date and UTC Datetime(us). Core and
Python random fixtures differ, so their timings cannot isolate adapter overhead.
The original native run also measures prefix rank, dynamic prefix rank and joins;
all formulations, plans and samples remain linked below. No cross-run speedup
is implied.

Joins have a **2,000,000 intermediate-row safety cap**, including self-matches;
missing timings are skips. Even disjoint 3M-row inputs exceed it. Group equality
joins use a conservative group-size bound; coordinate-band joins include their
bounded-fixture encoding in timing. Native rank avoids pair materialization and
has comparable asymptotic complexity; these Polars rank APIs were marked unstable
at the measured revision.

**Peak live buffer capacity**, including output, reaches 137 MiB for packed
Fenwick, 91.6 MiB for indirect Fenwick and 160 MiB for the segment tree at 3M
distinct ends. This is neither requested-heap instrumentation nor RSS; native
process memory was not measured. Small fixtures use independent nested-loop
oracles; full-sized candidates and timed outputs are cross-checked. See
[validation and memory details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md)
and the shared [methodology](benchmarking.md).

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench containment --locked
uv run --locked --no-sync python benchmarks/containment_count.py --methods plugin rank_runs --output target/containment-rank-runs.json
```

Use the shared release setup first. To reproduce the original native selection,
use `--methods plugin rank rank_by join bands equality` and a separate output.
`CONTAINMENT_SIZES` and `CONTAINMENT_CSV` select kernel sizes and output;
the Python runner accepts `--sizes` and `--repeats`.

</details>

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv) ·
[Original native samples, skips, plans and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-windows.json) ·
[Run-rank samples and metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json) ·
[Design notes and historical comparisons](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/containment.md)
