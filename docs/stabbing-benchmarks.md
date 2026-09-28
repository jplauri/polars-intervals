# Minimum stabbing points benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_stabbing_points`](api.md#polars_intervals.minimum_stabbing_points)
finds the fewest points hitting every interval. Production's direct scan of
end-sorted inputs saves both sorting time and record storage; packed sorting is
its fallback for other input. Indirect sorting is slower on the largest shuffled
cases but wins some reverse-order workloads and uses fewer temporary bytes for
Int64 endpoints.

## Results

**Polars collection · 24 Polars threads, single-threaded solver · median of 3
samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json)**

--8<-- "docs/assets/benchmarks/stabbing-polars-table.md"

Collection includes plugin dispatch, validation, physical adaptation and logical
list construction. Fixture construction, casts and checks are outside timing.
Date uses narrower physical records than Int64 and microsecond Datetime. These
regular dense fixtures differ from the random dense core cases below; subtracting
their times would not isolate adapter overhead.

**Rust core · single-threaded · median of 3 samples · production has no phase
clocks; candidate totals include them · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json)**

Int64 endpoints; production detects end-sorted input before its packed fallback.
The references always sort packed records or original-row indices.

--8<-- "docs/assets/benchmarks/stabbing-core-table.md"

Input order changes the result more than small differences between production
and the packed reference. Indirect sorting's reverse-order wins remain relevant;
the packed fallback was selected for its broader shuffled-input performance.
The instrumented sortedness-detection candidate is preserved in the samples,
without adding another almost identical production series to the main table.
Phase-clock overhead matters most at the smallest sizes.

For 3M disjoint Int64 rows, production peak **requested live heap** is 32.0 MiB
when sorted and 77.8 MiB when shuffled. The extra storage is the packed record
buffer. Output size matters too: a sorted common-intersection case allocates only
32 bytes, since it emits one point. These core allocations include output-vector
capacity and are not plugin process RSS.

## Coverage and limitations

The core covers 1K, 10K, 100K, 1M and 3M rows, twelve geometry families, and
end-sorted, reverse and shuffled orders. Int64 spans the full matrix; Date's
physical-width supplement covers disjoint, dense and identical families.
Equal-end and identical families remain end-sorted even when shuffled or reversed,
so their order labels still exercise the fast path. Tied ends also affect sorting
behavior in random reversed fixtures.

The installed release-wheel suite measures Int64, Date and microsecond Datetime
on disjoint, dense and identical inputs, sorted and shuffled. Other Datetime
units, timezone metadata, chunk adaptation and grouping have correctness tests
but no matching performance matrix here. No native Polars expression baseline
is supplied.

Core outputs are checked for coverage, sorted uniqueness and optimum cardinality
against an independent right-to-left maximum-packing oracle. Small coordinate-
and interval-subset oracles cross-check it before timing. Temporal checks use
coverage joins and explicit disjoint-packing certificates, rather than replaying
the production greedy rule. [Design and validation details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#minimum-stabbing-points)
retain the proof and oracle boundaries.

Core totals include validation, materialization, sorting, scanning and temporary
buffer destruction; output destruction is excluded. Blank production phase
fields mean unavailable. Cleanup changed the driver and tests, while the
measured production and candidate implementations remained unchanged.

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench minimum_stabbing_points --locked > benchmarks/results/stabbing-local.csv
python -I /path/to/checkout/benchmarks/stabbing_temporal.py > stabbing-temporal-local.csv
uv run --no-sync python benchmarks/stabbing_summary.py
```

`STABBING_BENCH_MAX` and `STABBING_BENCH_SAMPLES` restrict the core run.
Run the temporal command with the installed release wheel's Python from outside
the checkout; the summary command reads the published files.

</details>

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-temporal-windows.csv) ·
[Metadata and historical validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#minimum-stabbing-points)
