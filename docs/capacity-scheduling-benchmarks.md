# Capacity scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`max_weight_with_capacity`](api.md#polars_intervals.max_weight_with_capacity)
selects intervals with the highest total weight while limiting how many may
overlap at once. In synthetic examples with small, independent overlap groups,
full Polars queries took **28–119 ms for one million integer intervals** and
returned an exact optimum. Polars has no built-in solver for this optimization
problem. Large, interconnected groups can take much longer.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/capacity-summary.md:3:-3"

Each overlap group has 32 intervals. A limit of 31 still requires choosing one
to leave out, while a limit of 32 allows all intervals in these examples and
makes the task easier.

<details markdown="1">
<summary>Benchmark details</summary>

<span id="full-polars-measurements"></span>

**What was compared**

The headline table uses integer endpoints at one million rows and shows overlap
limits from selecting one interval per group through selecting all 32.

--8<-- "docs/assets/benchmarks/capacity-summary.md:-2:"

Each set contains 32 intervals that overlap one another, with no overlap between
sets. Capacity is the maximum number that may be selected at the same time.
The columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/capacity-polars-table.md"

Runtime does not simply increase with capacity. A capacity of 31 still requires
choosing which interval to leave out. At 32, every interval in these positive-
weight examples can be selected, making the task much easier.

**Settings**

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

**Complete Polars queries · solver uses up to 8 workers · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

Small cases are checked by trying all subsets. Larger results are checked for
capacity violations and compared with independent optimization implementations.
The Polars examples also have independently calculable best weights.

Query times include validation and output construction, but exclude generating
and converting input data.

<span id="coverage-and-limitations"></span>

**Limitations**

Algorithm tests cover empty inputs through one million rows, varied overlap,
sorted and shuffled order, seven weight distributions and different capacity
limits. Polars tests cover integer, Date and Datetime endpoints, including
timezone-aware nanosecond timestamps. No equivalent Polars-only optimizer was
benchmarked.

The algorithm-only datasets differ from the Polars examples. Some expensive
alternative-method combinations were omitted, and the largest inputs have
narrower test coverage.

<span id="reproduce-and-data"></span>

<span id="operation-specific-commands"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity --locked > benchmarks/results/capacity-local.csv
python -I /path/to/checkout/benchmarks/capacity_temporal.py > capacity-temporal-local.csv
```

`CAPACITY_BENCH_MAX_N`, `CAPACITY_BENCH_SAMPLES` and `CAPACITY_BENCH_FAMILY`
restrict the core run. Run the temporal command with the installed release
wheel's Python from outside the checkout, following the shared setup.

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv) ·
[Omission log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.log) ·
[Cleanup samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-cleanup-windows.csv) ·
[Design and historical validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection)

<span id="underlying-algorithm-comparisons"></span>

<span id="historical-validation"></span>

**History**

The saved run record names base revision
`cd3a0181232bda9a2c1f7b0aafdfef1adcd8233f`. The measured package
version was not recorded. The original core source hash matches `29d75ee`,
before `cee8e52` removed the per-interval edge-index vector and intermediate
worker-result buffering. Those changes affect allocations and reconstruction
inside the timed solver. The separately recorded cleanup run measures core
calls, not the full Polars queries in the headline table.

**Rust algorithm only · package uses up to 8 workers · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

These inputs are shuffled and have positive weights. The package can solve
independent overlap sets in parallel. The alternatives solve those sets one at
a time, solve the entire input together, or use a general-purpose optimizer.
All return an exact optimum.

--8<-- "docs/assets/benchmarks/capacity-core-table.md"

One million rows in independent overlap sets take 128 ms at capacity two.
A dense overlapping dataset of the same size takes 1.09 seconds. Parallel
processing helps independent sets, but the alternatives win on some dense or
identical-interval inputs.

On the million-row independent-set example, peak requested heap storage is
96.4 MiB for the package versus 56.6 MiB when processing sets one at a time.
The extra memory helps reduce runtime. These are algorithm allocations, not
whole-process memory.

The displayed run predates a memory-allocation cleanup. A separate before/after
run is retained, but does not establish a speedup from that change. Details and
omissions are in the [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection).

</details>
