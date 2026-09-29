# Capacity scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_weight_with_capacity`](api.md#polars_intervals.max_weight_with_capacity)
selects intervals with the highest possible total weight while limiting how
many selected intervals can overlap at once. Complete Polars queries took
**28–119 ms for one million integer intervals** in the displayed examples,
which consist of separate sets of 32 overlapping intervals. The result is
exact. Large interconnected sets of overlapping intervals can take much longer
than these small independent sets.

## Results

**Complete Polars queries · solver uses up to 8 workers · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

Each set contains 32 intervals that overlap one another, with no overlap between
sets. Capacity is the maximum number that may be selected at the same time.
The columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/capacity-polars-table.md"

Runtime does not simply increase with capacity. A capacity of 31 still requires
choosing which interval to leave out. At 32, every interval in these positive-
weight examples can be selected, making the task much easier.

<details markdown="1">
<summary>Underlying algorithm comparisons</summary>

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

</details>

## Coverage and limitations

Algorithm tests cover empty inputs through one million rows, varied overlap,
sorted and shuffled order, seven weight distributions and different capacity
limits. Polars tests cover integer, Date and Datetime endpoints, including
timezone-aware nanosecond timestamps. No equivalent Polars-only optimizer was
benchmarked.

Small cases are checked by trying all subsets. Larger results are checked for
capacity violations and compared with independent optimization implementations.
The Polars examples also have independently calculable best weights.

Query times include validation and output construction, but exclude generating
and converting input data. The algorithm-only datasets differ from the Polars
examples. Some expensive alternative-method combinations were omitted, and the
largest inputs have narrower test coverage.

The displayed run predates a memory-allocation cleanup. A separate before/after
run is retained, but does not establish a speedup from that change. Details and
omissions are in the [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection).

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity --locked > benchmarks/results/capacity-local.csv
python -I /path/to/checkout/benchmarks/capacity_temporal.py > capacity-temporal-local.csv
```

`CAPACITY_BENCH_MAX_N`, `CAPACITY_BENCH_SAMPLES` and `CAPACITY_BENCH_FAMILY`
restrict the core run. Run the temporal command with the installed release
wheel's Python from outside the checkout, following the shared setup.

</details>

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv) ·
[Omission log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.log) ·
[Cleanup samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-cleanup-windows.csv) ·
[Design and historical validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection)
