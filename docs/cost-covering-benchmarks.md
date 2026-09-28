# Minimum-cost covering benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cost_cover`](api.md#polars_intervals.minimum_cost_cover) selects a
continuous cover minimizing cost, then interval count. Production's Fenwick
dynamic program uses less memory than the segment-tree reference and is faster
on many measured workloads with many reachable frontiers. The Date-width chain
is a meaningful exception; neither tree wins every case.

## Results

**Polars collection · single-threaded solver, Polars pool size unrecorded · median
of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

--8<-- "docs/assets/benchmarks/cost-cover-polars-table.md"

Installed release-wheel measurements include eager selection, Series retrieval,
plugin validation, physical adaptation and output construction. Fixture creation,
expression construction and casts are outside timing. The native fixtures and
costs differ from the Rust harness: these totals cannot be subtracted from core
times to estimate adapter overhead.

**Rust core · single-threaded · median of 3 samples · internal phase clocks included
· [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

Shuffled inputs with random nonnegative costs:

--8<-- "docs/assets/benchmarks/cost-cover-core-table.md"

Fenwick uses suffix-minimum frontier queries; the segment tree supports general
range minima. Both stop ancestor updates when a value does not improve. Their
performance depends on the number and arrangement of useful frontier states:
irrelevant-row workloads mostly pay validation and output costs. A direct scan
of previous states is also recorded, but deliberately limited to 1K rows because
its work can be quadratic.

The segment tree wins the Date-width chain. Its nearly equal irrelevant-row
median does not establish a reliable advantage. The memory tradeoff favors
Fenwick on the larger state spaces: peak **requested live heap** for the 1M
Int64 chain is 105 MB versus 158 MB for the segment tree; dense overlap uses
87.2 MB versus 152 MB. These decimal MB measurements include output and temporary
buffers, not process RSS.

## Coverage and limitations

The shared [covering harness](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering-methodology.md)
spans 1K–1M rows, 17 geometry/difficulty families, three orders and selected cost
distributions. It includes zero costs, skew, ties, duplicate costs, expensive
long intervals, gaps and infeasible targets. Date and Datetime physical-width
slices cover chains, dense overlaps and duplicates. The separate plugin suite
covers eight integer/temporal dtypes on four families, with sorted and shuffled
rows. Nearly sorted physical-width cases and a native Polars optimization
baseline are absent.

Core correctness checks combine independent coverage verification with objective
agreement across implementations. Exhaustive subset optimality checks apply to
small test cases; the quadratic reference stops at 1K benchmark rows. Release-wheel
checks verify coverage and deterministic masks, not independent full-size
optimality. The shared notes retain the exact oracle and property-test coverage.

Core totals include preprocessing, dynamic programming, reconstruction,
temporary-buffer destruction and phase-clock overhead; phase sums need not equal
the total. Allocation measurements use a separate untimed invocation. Early
failures can skip later phases. The recorded wheel predates adapter cleanup;
original and cleanup hashes are retained separately, and the measured solver and
candidate algorithms are unchanged.

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
python -I /path/to/checkout/benchmarks/covering_temporal.py > covering-temporal-local.csv
```

The target measures both covering operations. `COVER_BENCH_MAX` and
`COVER_BENCH_SAMPLES` restrict it. Run the final command with the installed release
wheel's Python from outside the checkout, following the shared setup guide.

</details>

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv) ·
[Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#covering) ·
[Validation and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)
