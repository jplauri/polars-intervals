# Minimum-cost covering benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`minimum_cost_cover`](api.md#polars_intervals.minimum_cost_cover) covers a
target range without gaps while minimizing the total cost of selected intervals.
Full Polars queries took **211–263 ms for one million shuffled integer
intervals** in the two synthetic examples below. The result is exact. Polars has
no built-in solver for this optimization problem. The number and arrangement of
useful intervals affect runtime and working memory.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/cost-cover-summary.md:3:-3"

These million-row examples take roughly a quarter of a second. Having many
overlapping alternatives takes longer than covering a continuous chain.

Measurements used a September 26, 2026 build labeled 0.1.0. Input checks have
since changed. The updated full Polars query has not been timed.

<details markdown="1">
<summary>Benchmark details</summary>

**Measured build**

The saved measurement record is dated `2026-09-26T15:21:30.752129+00:00`, with
base revision `cee8e52e96a9a260af712d84fb86574c6225d309`. Artifact hashes identify
an explicitly recorded `polars_intervals-0.1.0-cp314-cp314-win_amd64.whl`.
The measured core source hash matches the covering solver in `1f27171`.
Metadata records the later adapter cleanup separately, including common plugin
input validation and fewer scalar copies. The record does not isolate the
runtime effect of those adapter changes. Harness verification and CSV summary
cleanup are outside the timed query, and no cleanup speedup is claimed.

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

The headline table compares the two measured shuffled integer workloads at one
million rows: a continuous chain and many overlapping alternatives.

--8<-- "docs/assets/benchmarks/cost-cover-summary.md:-2:"

<span id="operation-notes"></span>

**Operation notes**

If costs tie, the operation selects fewer intervals.

<span id="full-polars-measurements"></span>

**Full Polars measurements**

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

<span id="underlying-algorithm-and-memory-comparisons"></span>

**Underlying algorithm and memory comparisons**

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

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

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

<span id="reproduce-and-data"></span>

**Reproduce and data**

<span id="operation-specific-commands"></span>

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
python -I /path/to/checkout/benchmarks/covering_temporal.py > covering-temporal-local.csv
```

The target measures both covering operations. `COVER_BENCH_MAX` and
`COVER_BENCH_SAMPLES` restrict it. Run the final command with the installed release
wheel's Python from outside the checkout, following the shared setup guide.

[Setup, metrics and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv) ·
[Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#covering) ·
[Validation and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)

</details>
