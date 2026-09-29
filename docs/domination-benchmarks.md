# Minimum-cost dominating-set benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cost_dominating_set`](api.md#polars_intervals.minimum_cost_dominating_set)
selects the cheapest representatives so every interval is selected or overlaps
a selected interval. Equal-cost solutions use the fewest rows. On a synthetic
chain of **100,000 shuffled integer intervals**, complete Polars queries took
**7.97 ms without a cost column** and **18.0 ms with varying costs**. Each interval
in this chain overlaps only its immediate neighbors. Uniform costs allow a
faster greedy algorithm. Both modes return an exact optimum, but overlap
patterns and input order affect runtime.

## Results

**Complete Polars queries · one Polars thread · median of 5 samples · seed 7 ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-polars-windows-20260929.metadata.json)**

“No cost column” assigns unit costs. Supplying ones asks for the same objective.
All-zero costs still require the fewest representatives. “Varying costs” includes
zero and positive values. Grouped row counts are totals across 32 independent
chains. Integer endpoints use Int64, and UTC datetimes retain nanosecond precision.

--8<-- "docs/assets/benchmarks/domination-polars-table.md"

Query times include optimization, validation, input extraction, selection and
Boolean output. Building inputs and queries is excluded. Tiny grouped inputs
show the overhead of solving each group. When all intervals overlap one another,
only one representative is needed, and this date example is faster than the chain.

<details markdown="1">
<summary>Underlying algorithm and memory comparisons</summary>

**Rust algorithm only · one thread · median of 5 samples · seed 7 ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-production-windows-20260929.metadata.json)**

Complete calls include
validation, sorting, optimization, allocation, reconstruction and internal
cleanup. Returned-mask destruction and correctness checks are excluded.
The package uses greedy selection for uniform costs and a heap prefix solver
otherwise. Alternatives transform overlap demands into a covering problem,
either calling the existing cover solver or sharing its preparation work.
The heap comparison uses that solver even on uniform-cost inputs.

--8<-- "docs/assets/benchmarks/domination-core-table.md"

The million-row [final-production repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-final-large-windows-20260929.metadata.json)
found production versus fused-cover medians of 193/255 ms for shuffled disjoint rows,
209/298 ms for mixed-cost paths, and 125/207 ms for expensive-hub stars
(Int64, seed 7). The corresponding requested live heap was about 66/122 MB.
Full cliques took 205/169 ms and 116/66 MB. Deep nesting took 57.8/34.0 ms
and 75.3/49.2 MB. These are meaningful losses for the package algorithm.

Direct greedy reduced a million-row sorted unit path to 10.1 ms versus
96.3 ms for fused covering. Explicit ones/zeros on 100k shuffled paths also
benefited. Reversed duplicate geometries are a visible unit-greedy loss
against fused covering. The earlier candidate comparison showed the same pattern.
Runs and seeds remain separate. Small differences do not establish a reliable winner.

</details>

## Coverage and limitations

The synthetic inputs cover two seeds, integer and temporal endpoints, five row
orders, uniform and varying costs, empty rows, extreme coordinates and disconnected
groups. Polars also covers mismatched chunks and streaming. **Polars timings stop
at 100,000 rows**. Million-row measurements cover only the Rust algorithm.

Every timed output is checked outside timing. Original-graph subset oracles
cover tiny instances; suitable families have analytic optima. Other core rows
use candidate objective agreement plus independent domination feasibility;
large nonanalytic weighted Polars rows are explicitly `feasibility_only`.
[Proofs and verification details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/domination-notes.md)
distinguish these guarantees.

Memory is [**requested live heap**](benchmarking.md#memory-metrics), excluding inputs,
allocator overhead, stack and the rest of the process. At one million rows the
package requested about 66 MB versus 122 MB for fused covering on weighted chains,
but 116 MB versus 66 MB when all intervals overlap. Results describe the
[shared Windows machine](benchmarks.md#hardware). Other operating systems and
whole-process memory were not measured.

The saved measurements predate the shared-helper cleanup. The selected algorithms
are unchanged. [Source and cleanup notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/domination-notes.md#measurement-sources-and-cleanup)
identify the measured build and later changes.

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
