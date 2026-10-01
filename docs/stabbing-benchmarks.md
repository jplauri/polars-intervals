# Minimum stabbing points benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`minimum_stabbing_points`](api.md#polars_intervals.minimum_stabbing_points)
finds the fewest points needed so that every interval contains at least one
selected point. Complete Polars queries took **under 140 ms for three million
intervals**. The result is exact. Input order has a large effect because
intervals already sorted by their ends need less processing. Polars has no
built-in solver for this optimization problem.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/stabbing-summary.md:3:-3"

The three-million-row examples are faster when sorted, and overlaps reduce
how many points must be returned. At 1,000 non-overlapping rows, shuffled
input is faster, though both queries take under 1 ms.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

--8<-- "docs/assets/benchmarks/stabbing-summary.md:-2:"

The compact table selects integer inputs at 1,000 and three million rows,
with sorted and shuffled orders; only the larger size also includes regular
overlaps.

Non-overlapping intervals each need their own point. In the regular overlapping
inputs, one point can serve several intervals. Sorted rows are ordered by their
ends. Columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/stabbing-polars-table.md"

The shuffled three-million-row examples take 79–139 ms across these endpoint
types. Already sorted inputs take 4–28 ms. Overlap also changes the amount of
output to create, so input row count alone does not determine runtime.

**Settings**

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

**Complete Polars queries · one solver thread, 24 Polars threads · median of
3 samples · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json)**

Every output is checked to hit all intervals. A separate algorithm computes the
minimum number of points using a different approach. Small exhaustive tests
check both implementations. Polars examples also include independently
verifiable evidence that fewer points cannot suffice.

Timings include validation, processing and output creation. Generating and
converting inputs is excluded.

<span id="coverage-and-limitations"></span>

**Limitations**

Algorithm tests cover 1,000 to three million rows, twelve interval patterns,
and sorted, reversed and shuffled orders. Integer endpoints have the widest
coverage. Polars tests also cover Date and microsecond Datetime endpoints on
non-overlapping, overlapping and identical intervals.

Algorithm-only and Polars inputs differ, so their times cannot be used to
calculate Polars overhead. Equal endpoints can leave data effectively sorted
even after shuffling.

Other datetime units, timezones and grouping have correctness tests but no
matching performance measurements here. No Polars-only alternative was measured.
The [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#minimum-stabbing-points)
retain the algorithms, correctness checks and exact measurement boundaries.

<span id="reproduce-and-data"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench minimum_stabbing_points --locked > benchmarks/results/stabbing-local.csv
python -I /path/to/checkout/benchmarks/stabbing_temporal.py > stabbing-temporal-local.csv
```

`STABBING_BENCH_MAX` and `STABBING_BENCH_SAMPLES` restrict the core run.
Run the temporal command with the installed release wheel's Python from outside
the checkout.

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-temporal-windows.csv) ·
[Metadata and historical validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#minimum-stabbing-points)

<span id="historical-validation"></span>

**History**

**Rust algorithm only · one thread · median of 3 samples · integer endpoints ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json)**

The package detects intervals that are already sorted by their ends. Both
alternatives always sort, using either interval records or row indices.
Their times include internal instrumentation that the package call does not.

--8<-- "docs/assets/benchmarks/stabbing-core-table.md"

The package benefits from sorted input. Sorting row indices can be faster on
reverse-ordered data, but is slower on the largest shuffled examples. On three
million non-overlapping integer intervals, the package's peak requested heap
storage is 32.0 MiB when sorted and 77.8 MiB when shuffled. This includes output
and sorting buffers, not whole-process memory.

</details>
