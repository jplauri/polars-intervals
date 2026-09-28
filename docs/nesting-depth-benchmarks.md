# Nesting depth benchmarks

## Summary

[`nesting_depth`](usage.md#nesting-depth) computes each row's longest strict
containment chain using a packed vector frontier. Complete Polars collection
took about 200 ms for three million shuffled Int64 rows in the measured shallow
and deep families; avoiding coordinate compression improves core runtime, but
deep chains need more memory and identical intervals favor an indirect layout.

## Results

**Polars collection · 4 threads · median of 5 samples ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.json)**

--8<-- "docs/assets/benchmarks/nesting-collection-table.md"

Sorted input is substantially cheaper for these fixtures. More groups do not
necessarily make a query faster: group dispatch and scattering are included,
and row counts above are totals across groups. Date uses i32 physical endpoints;
Int64 and Datetime use i64. No equivalent native Polars baseline was measured.

**Rust core · single-threaded · median of 5 samples · i64, one group ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.json)**

--8<-- "docs/assets/benchmarks/nesting-core-table.md"

These candidates include internal phase clocks; the frontier candidate represents
the production design. A separate [uninstrumented production repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-windows.csv)
supports the choice, with a 215 ms full-chain median at three million rows.
The indirect Fenwick candidate remains faster on identical intervals. The
sortedness-scan experiments did not justify an additional production path;
close repeat differences do not establish a reliable winner.

## Coverage and limitations

The core matrix spans 1K–3M rows, twenty structures, depth, duplicates, ties,
empties, containment density and input order, plus bounded Date-physical and
serial grouping cases. Collection covers Int64, Date and Datetime(us), five
families, four orders and 1–1,000 groups. Core and collection generators differ;
subtracting these timings would not isolate adapter overhead.

Collection includes lazy optimization/planning, execution and Series retrieval;
fixture construction, casts, expression construction and validation are outside
timing. Serial grouped core calls are distinct from actual Polars windows.
Small inputs use independent quadratic oracles; full core outputs must agree
across candidates, while full collection outputs match analytic row-aligned
depths. [Validation details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#validation-details)
include exhaustive and frontier-invariant checks.

At three million rows, **peak requested live heap** for the frontier is
91.6 MiB on shallow input and 124 MiB on a full chain, versus 114 MiB for packed
Fenwick and 68.7 MiB for indirect Fenwick. Geometric frontier growth causes the
deep-chain penalty. Allocation measurements are separate untimed calls, repeated
in the timing rows rather than independently sampled; Polars memory is unmeasured.
See the shared [metric definitions and methodology](benchmarking.md).

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```powershell
$env:NESTING_SAMPLES = "5"
cargo bench -p intervals-core --bench nesting_depth --locked
$env:POLARS_MAX_THREADS = "4"
uv run --no-sync python benchmarks/nesting_depth.py --output target/nesting-polars.csv
```

The recorded collection run used an externally installed release wheel.
`NESTING_SIZES`, `NESTING_SCENARIOS` and `NESTING_METHODS` select core cases;
the current default includes the uninstrumented production entry point.
The original eight-candidate matrix and the repeat use different selections,
recorded in their metadata.

</details>

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv) ·
[Collection samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv) ·
[Repeat metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-windows.json) ·
[Recurrences, sorted-input investigation and historical evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#nesting-depth)
