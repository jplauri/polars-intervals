# Capacity scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_weight_with_capacity`](api.md#polars_intervals.max_weight_with_capacity)
maximizes selected weight under an overlap limit. Independent components and
sufficient-capacity checks give production its largest measured gains; dense
constrained inputs can favor a single flow network. Parallel components reduce
wall time at the cost of additional live memory.

## Results

**Polars collection · solver uses up to eight workers · Polars pool size unrecorded
· median of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

--8<-- "docs/assets/benchmarks/capacity-polars-table.md"

These release-plugin workloads are independent 32-row cliques. Collection,
adapter validation and output creation are timed; construction and casts are
outside timing. The sharp drop at capacity 32 comes from selecting all useful
intervals without flow. These fixtures differ from the core suite below, so
subtracting the timings would not measure adapter overhead.

**Rust core · serial baselines, production up to eight workers · median of 3
samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

Shuffled rows with positive weights; all core preprocessing and output creation
are included, with phase clocks disabled during timing.

--8<-- "docs/assets/benchmarks/capacity-core-table.md"

Whole flow uses one specialized network; serial components solve each independent
overlap component separately. Generic flow is an independent adjacency-list
implementation. Core component fixtures have peak concurrency 16. Decomposition
and parallelism help separated work, while production's extra checks can lose on
dense input. Capacity one delegates to the existing weighted scheduler: on 1M
component rows it took 121 ms versus 107 ms for forced parallel flow, another
measured loss. Highly compressed identical-interval networks can favor generic
flow, as the capacity-two table also shows.

Peak **requested live heap** for 1M component rows at capacity two was 96.4 MiB
for production versus 56.6 MiB for serial components. Parallelism trades extra
row and mask buffers for lower wall time; these are core allocations, not RSS.

## Coverage and limitations

The bounded core matrix spans empty input through 1M rows, geometry, sorted and
shuffled order, seven weight distributions, and capacities around measured
concurrency. It deliberately omits 182 expensive forced-flow combinations using
`n * min(k, peak) <= 2,000,000`; larger sizes use fewer weight distributions.
No native Polars optimization baseline is supplied. Temporal coverage includes
Int64, Date, microsecond Datetime and UTC nanoseconds.

All candidates were checked for feasibility and optimum agreement. Small
restrictions use exhaustive subsets; large non-structural cases rely on
independent flow-engine agreement. Temporal cliques use an independent top-k
oracle. [Validation details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection)
describe those boundaries.

These tables retain the original run, before an allocation cleanup. The separate
330-workload before/after run reduced allocations, but variation also affected
the unchanged generic engine: **no cleanup speedup is claimed**. Its samples and
source hashes remain separate from the original results. Raw phase fields come
from the separate allocation call, repeated on timing rows; zero means unavailable.

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
