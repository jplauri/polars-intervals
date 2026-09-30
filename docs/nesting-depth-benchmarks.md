# Nesting depth benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`nesting_depth`](api.md#polars_intervals.nesting_depth) measures how deeply each
interval sits inside a chain of other intervals. Outermost intervals have depth
zero. In synthetic benchmarks, full Polars queries took **about 200 ms for three
million shuffled integer intervals**, with either shallow or deep nesting. The
sorted, fully nested example took **43 ms**. Input order and grouping can matter
as much as nesting depth. A native Polars comparison has not been measured.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/nesting-summary.md:3:-3"

Sorting makes a large difference in the fully nested example. Splitting 100,000
rows into 1,000 groups roughly doubles runtime because each group adds work.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and endpoint types**

The headline cases contrast shallow and deep nesting, smaller and larger inputs,
and the effects of sorting or splitting the input into many groups.

The Polars run used four Polars threads, one warmup and five samples per case,
with seed 731. The displayed times are medians. Settings, software versions and
source hashes are recorded in the
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.json).
No native Polars implementation was measured. The summary uses Int64 endpoints.

--8<-- "docs/assets/benchmarks/nesting-summary.md:-2:"

“Depth up to 7” is shallow nesting. “Fully nested” means each successive
interval is inside another, producing a long chain. Row counts are totals across
all groups. The columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/nesting-collection-table.md"

Sorting makes a large difference in the fully nested example. Splitting 100,000
rows into 1,000 groups increases runtime in the shallow example because each
group adds processing overhead. Date columns were faster in these measurements,
but the table does not establish that they will always be faster.

**Underlying algorithm comparisons**

**Rust algorithm only · one thread · median of 5 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.json)**

The package's frontier design tracks the ends of containment chains. The two
Fenwick alternatives use tree-based counting structures. These comparisons
include internal timing instrumentation and exclude Polars.

--8<-- "docs/assets/benchmarks/nesting-core-table.md"

The package's design has a large advantage on fully nested inputs. An alternative
is slightly faster when every interval is identical. A separate direct run of
the package's Rust function took 215 ms for three million fully nested rows.
Its peak requested heap allocation was 91.6 MiB on shallow input and 124 MiB on
a full chain. Deep nesting requires more working storage.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

Algorithm tests cover twenty patterns with 1,000 to three million rows. They
include shallow and deep nesting, duplicates, shared endpoints, empty intervals
and different input orders. Polars tests cover integer, Date and Datetime
endpoints and up to 1,000 groups.

Small cases are checked by direct comparisons between intervals. Larger
algorithm outputs are compared across implementations. Polars examples use
datasets whose correct depths can be calculated independently.

Query times include planning, execution and retrieving the result. Creating
the input data and converting its types are excluded. Algorithm-only and Polars
datasets differ, so their times do not isolate Polars overhead. No equivalent
Polars-only expression was benchmarked, and whole-query memory was not measured.
See the [validation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#validation-details)
and [memory definitions](benchmarking.md#memory-metrics).

<span id="reproduce-and-data"></span>

**Reproduce and data**

```powershell
$env:NESTING_SAMPLES = "5"
cargo bench -p intervals-core --bench nesting_depth --locked
$env:POLARS_MAX_THREADS = "4"
uv run --no-sync python benchmarks/nesting_depth.py --output target/nesting-polars.csv
```

The recorded collection run used an externally installed release wheel.
`NESTING_SIZES` and `NESTING_SCENARIOS` select algorithm tests. The runner now
times only direct calls to the package's Rust function, without internal timing
instrumentation. The original comparison and the repeat selected different
candidate methods, recorded in their measurement settings. See the
[provenance guidance](benchmarking.md#measurement-rules) for the archived suite.

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv) ·
[Collection samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv) ·
[Repeat metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-windows.json) ·
[Recurrences, sorted-input investigation and historical evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#nesting-depth)

</details>
