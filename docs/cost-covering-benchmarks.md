# Minimum-cost covering benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`minimum_cost_cover`](api.md#polars_intervals.minimum_cost_cover) covers a target
range without gaps while minimizing the total cost of selected intervals.
If costs tie, it selects fewer intervals. Complete Polars queries took
**211–263 ms for one million shuffled integer intervals** in the two displayed
examples. The result is exact. The number and arrangement of useful intervals
affect both runtime and working memory.

## Results

**Complete Polars queries · one solver thread · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

The first input forms a continuous chain of intervals whose endpoints touch.
The second has many overlapping alternatives for covering the target. Both are
shuffled. Columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/cost-cover-polars-table.md"

These million-row examples take roughly a quarter of a second. The overlapping
case takes longer than the chain for each endpoint type. Date values are
somewhat faster in this run, but the measurements do not separate type-handling
costs from the rest of the computation.

<details markdown="1">
<summary>Underlying algorithm and memory comparisons</summary>

**Rust algorithm only · one thread · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)**

The package and its alternative use different tree structures to track the
cheapest way to reach each part of the target. Their technical names are Fenwick
tree and segment tree. These examples use shuffled input and random nonnegative
costs. Timings include internal measurement overhead. “32-bit endpoints” uses
the storage width of Date values without calling Polars.

--8<-- "docs/assets/benchmarks/cost-cover-core-table.md"

The package's tree is faster on most of these examples, but the alternative
wins on the chain with 32-bit endpoints. It also uses more memory: on the
million-row integer chain, peak requested heap storage is 158 MB compared with
105 MB for the package. These figures include output and working buffers,
not total process memory.

</details>

## Coverage and limitations

The shared covering tests span 1,000 to one million rows and seventeen input
patterns. They include different input orders, duplicate intervals, gaps,
impossible covers, zero costs, tied costs and expensive intervals competing with
cheaper ones. The separate Polars tests cover eight integer and temporal
endpoint types on four patterns. No Polars-only optimizer was benchmarked.

Results are checked for complete target coverage and agreement on optimal cost
across implementations. Small tests try every possible subset. A simpler
reference algorithm is limited to 1,000 benchmark rows. Polars tests check
coverage and repeatable selections, without independently proving optimality
for every large input.

Query times include validation, optimization and output construction. Input
generation, query construction and type conversion are excluded. The measured
package predates an input-handling cleanup, with the optimizer unchanged.
Algorithm-only inputs and costs differ from the Polars examples, so their
timings cannot isolate Polars overhead. See the
[shared validation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering-methodology.md)
for the full checks and timing scope.

<span id="historical-validation"></span>

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_summary.py benchmarks/results/covering-local.csv
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
