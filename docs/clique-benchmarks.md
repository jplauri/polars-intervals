# Maximum-weight clique benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_weight_clique`](api.md#polars_intervals.max_weight_clique) selects the
highest-weight set of intervals that overlap one another. Without weights, it
selects the largest such set. On a benchmark with **one million shuffled integer
intervals**, complete Polars queries took **43.1 ms without weights** and
**140 ms with positive weights**. These synthetic inputs had at most eight
intervals overlapping at once. The result is exact in both modes, but overlap
patterns and weights strongly affect runtime. No native Polars alternative was
measured.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/clique-summary.md:3:-3"

Supplying positive weights makes the million-row integer example about three
times slower. When every interval overlaps every other one, the function can
select all useful rows without sorting: the unweighted Date example takes
2.06 ms, a special case rather than a general million-row runtime.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

--8<-- "docs/assets/benchmarks/clique-summary.md:-2:"

The compact table keeps three sizes of the shuffled integer input and adds
all-overlap Date, streaming Datetime and grouped integer cases, each with
unweighted and positive-weight runs.

**Full Polars measurements**

**Complete Polars queries · one Polars thread · median of 5 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.metadata.json)**

“No weights” uses the function's default. “All weights = 1” supplies a weight
column explicitly, asking for the same objective. “Positive weights” assigns
different positive values. “Positive and negative weights” also allows values
that make intervals undesirable to select. Int64, Date and UTC Datetime describe
the endpoint types. Grouped row counts are totals across groups.

--8<-- "docs/assets/benchmarks/clique-polars-table.md"

Omitting weights is useful when every row is equally valuable. In the
million-row integer example, supplying a column of ones increases runtime from
43.1 ms to 148 ms. Processing the unweighted data in 32 groups takes 49.6 ms.

Inputs where every interval overlaps every other interval are especially easy:
the function can select all useful rows without sorting. The million-row Date
example takes 2.06 ms without weights. It is a special case, not a general
million-row runtime.

**Underlying algorithm and memory comparisons**

**Rust algorithm only · one thread · median of 5 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-windows-20260928.metadata.json)**

The alternatives scan endpoints or keep track of active intervals in a heap.
“128-bit weights” matches the wide integer representation used inside the
plugin. “64-bit weights” is a narrower option for direct Rust callers.

--8<-- "docs/assets/benchmarks/clique-core-table.md"

The package wins strongly when all intervals overlap. Other methods can be
faster when few weighted intervals overlap: at one million such rows, packed
endpoint lists take 99.3 ms compared with 116 ms, but request 64 MB of heap
storage compared with 16 MB. Those allocations exclude caller inputs and the
rest of the process. A separate repeat shows the same tradeoff.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

Tests range from tiny inputs to one million rows. They cover different overlap
patterns, weights, row orders, duplicates, empty intervals and extreme integer
values. Polars tests also include dates, datetimes, groups and streaming input
stored in multiple chunks. Only selected input patterns reach one million rows.

Small tests compare against exhaustive subsets and direct interval comparisons.
Large benchmark outputs are independently checked for mutual overlap and
optimal total weight. Query times include validation, processing and creation
of the selection mask. Building inputs and queries is excluded.

These synthetic inputs on one machine show the effect of overlap and weights,
but do not predict every application. No Polars-only alternative is compared.
The [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/clique-notes.md)
retain the alternative algorithms, repeated runs and detailed validation.

<span id="reproduce-and-data"></span>

**Reproduce and data**

Raw [core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-windows-20260928.csv),
[repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-repeat-windows-20260928.csv),
[exploratory samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-exploratory-windows-20260928.csv),
and [Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.timings.csv)
remain separate. Each run has matching measurement settings and build details.
Table downloads retain exact medians, sample ranges and counts. The supporting
notes explain the independent correctness checks.

Follow [release setup](benchmarking.md#setup), then use new output filenames:

```powershell
$env:CLIQUE_CSV = "$PWD/benchmarks/results/clique-core-new.csv"
cargo bench -p intervals-core --bench max_weight_clique --locked
$env:POLARS_MAX_THREADS = "1"
uv run --no-sync python benchmarks/max_weight_clique.py --output benchmarks/results/clique-polars-new
```

`CLIQUE_SIZES`, `CLIQUE_CASES`, `CLIQUE_SEEDS`, `CLIQUE_DTYPES`,
`CLIQUE_WEIGHT_DTYPES`, `CLIQUE_SAMPLES`, and
`CLIQUE_WARMUPS` select focused core runs. Run metadata records the settings
actually used. Regenerate tables with the [reporting commands](benchmarking.md#generate-plots).

</details>
