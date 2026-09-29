# Minimum-cost dominating-set benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cost_dominating_set`](usage.md#select-a-minimum-cost-dominating-set)
uses an exact minimal-target reduction and heap prefix dynamic program for
general costs, with a direct cardinality greedy sweep for implicit units and
uniform explicit costs, including zero. Both take `O(n log n)` time and `O(n)`
space including validation, sorting and reconstruction. In complete-call
comparisons, heap DP won materially on weighted disjoint/path/star inputs;
covering remains better on some clique and nesting workloads.

## Results

**Release Polars collections · one Polars thread · median of 5 samples · seed 7.**
[Metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-polars-windows-20260929.metadata.json)
records source hashes and workload settings.

--8<-- "docs/assets/benchmarks/domination-polars-table.md"

These include plan optimization, validation/extraction, the algorithm and
Boolean output. Tiny and grouped rows expose expression and per-group overhead;
numeric, Date and exact Datetime values exercise the same typed adapter.

**Rust core · one thread · median of 5 samples · seed 7.** Complete calls include
validation, sorting, optimization, allocation, reconstruction and internal
cleanup. Returned-mask destruction and correctness checks are excluded.
[Final-production metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-production-windows-20260929.metadata.json)
identifies the measured source.

--8<-- "docs/assets/benchmarks/domination-core-table.md"

The million-row [final-production repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-final-large-windows-20260929.metadata.json)
found production versus fused-cover medians of 193/255 ms for shuffled disjoint rows,
209/298 ms for mixed-cost paths, and 125/207 ms for expensive-hub stars
(Int64, seed 7). The corresponding requested live heap was about 66/122 MB.
Losses matter: full cliques took 205/169 ms and 116/66 MB; deep nesting took
57.8/34.0 ms and 75.3/49.2 MB. Nesting has `m=1`; proper paths and full
proper cliques can have `m=n`. All nonempty candidates remain eligible.

Direct greedy reduced a million-row sorted unit path to 10.1 ms versus
96.3 ms for fused covering. Explicit ones/zeros on 100k shuffled paths also
benefited. Reversed duplicate geometries are a visible unit-greedy loss
against fused covering. The report retains these losses instead of tuning
dispatch thresholds. The earlier candidate comparison showed the same pattern.
Separate runs and seeds remain separate; sample ranges
are descriptive, and small differences do not establish a reliable winner.

## Coverage and limitations

The matrix spans tiny inputs through one million core rows, two seeds,
Int64/UInt64 endpoints, five orders, units/ones/zeros/mixed/skewed costs,
empty mixtures, sparse extrema, overlapping minimal targets and disconnected
components. Final production is remeasured after selection. Polars covers
numeric/temporal types, interleaved groups, mismatched chunks and streaming.
Polars timings stop at 100k rows; the million-row measurements are core-only.

Every timed output is checked outside timing. Original-graph subset oracles
cover tiny instances; suitable families have analytic optima. Other core rows
use candidate objective agreement plus independent domination feasibility;
large nonanalytic weighted Polars rows are explicitly `feasibility_only`.
[Proofs and verification details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/domination-notes.md)
distinguish these guarantees.

Memory is separately instrumented [**requested live heap**](benchmarking.md#memory-metrics),
not RSS or buffer capacity. It excludes inputs, allocator overhead and stack.
Buried expired heap proposals can consume `O(n)` memory. Results characterize synthetic
inputs on the [shared Windows machine](benchmarks.md#hardware), not every
application. No process RSS, other operating systems, literature challenger,
candidate coalescing or parallel optimizer was measured.

## Reproduce and data

<details markdown="1">
<summary>Reproduce this operation</summary>

Follow the [shared setup](benchmarking.md#setup). Use fresh output names:

```powershell
$env:DOMINATION_CSV = "$PWD/benchmarks/results/domination-new.csv"
$env:DOMINATION_SIZES = '0,1,8,32,1000,10000,100000,1000000'
$env:DOMINATION_SEEDS = '7,41'
$env:DOMINATION_SAMPLES = '5'
$env:DOMINATION_WARMUPS = '2'
cargo bench -p intervals-core --bench minimum_cost_dominating_set --locked
uv sync --locked --reinstall-package polars-intervals --config-setting 'build-args=--profile release'
$env:POLARS_MAX_THREADS = '1'
uv run --no-sync python benchmarks/minimum_cost_dominating_set.py --output benchmarks/results/domination-polars-new
```

Optional `DOMINATION_CASES`, `DOMINATION_METHODS`, and `DOMINATION_DTYPES`
select focused comparisons. Million-row cases are bounded by the runner's
matrix; quadratic DP is omitted above 64 rows. Restore thread settings after
the run. Run plot generation/reporting checks using [the shared commands](benchmarking.md#generate-plots).

</details>

Raw data and run-specific metadata: [pilot](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-pilot-windows-20260929.csv),
[large comparison](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-comparison-windows-20260929.metadata.json),
[final production](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-production-windows-20260929.csv),
[final million-row repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-final-large-windows-20260929.csv),
and [Polars collections](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-polars-windows-20260929.timings.csv).
