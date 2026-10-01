# Overlap counting benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`overlap_count`](api.md#polars_intervals.overlap_count) counts how many other
intervals overlap each row. In synthetic benchmarks, full Polars queries were
**at least 1.3× faster than Polars and took under 250 ms for three million rows**.
The advantage shrinks on small inputs split into many groups, where calling the
function adds overhead.

## Results

**Full Polars query time · milliseconds**

Native Polars is the fastest tested query using `search_sorted` expressions,
as-of joins, an endpoint sweep or an inequality join to count the same overlaps.

--8<-- "docs/assets/benchmarks/overlap-summary.md:3:-3"

The function's benefit is clearest on the larger inputs. At just 1,000 rows split
across 100 groups, search expressions and the function both take about 1 ms.
Those small differences do not establish a dependable winner.

Inputs made entirely of empty or nested intervals can favor native Polars.
Separate memory tests found 23–30% lower whole-process resident memory than the
lowest-memory native alternative with grouping, but 15–60% higher without it.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The headline cases show larger inputs with few or many overlaps, plus small
grouped inputs where the runtime difference is inconclusive.

Sparse inputs have relatively few overlaps. Dense inputs have many. The native
candidates use search expressions (`native_expr`), parallel search expressions (`native_parallel`),
as-of joins (`native_asof`), an endpoint sweep (`native_sweep`) or an inequality
join (`inequality_join`).

The summary uses the faster native median for each row. As-of joins win on the
three-million-row ungrouped examples. Search expressions win on the other
displayed examples. Speedup is native median divided by package median. Above 1
means the package is faster. Below 1 means it is slower.

--8<-- "docs/assets/benchmarks/overlap-summary.md:-2:"

The original detailed table keeps search expressions and as-of joins. All native
candidate samples remain in the linked source data.

--8<-- "docs/assets/benchmarks/overlap-headline.md"

**Settings**

The main run used 24 Polars threads, three warmups and nine samples per case,
with seed 42. The displayed times are medians. All displayed rows use shuffled
Int64 endpoints. Each group is processed independently, and the row count is
the total across all groups.

Times include query execution and creation of the result. Data generation and
query construction are excluded. See the shared [hardware](benchmarks.md#hardware).
Settings, software versions and source hashes are recorded in the
[main run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).

Results were checked against the alternative implementations, with additional
independent checks on edge cases.

<span id="coverage-and-limitations"></span>

**Limitations**

The main run covers 1,000 to three million rows, low and high overlap, sorted
and shuffled input, and one or 100 groups. The function also validates inputs,
while the Polars-only alternatives assume valid input.

Inputs consisting entirely of empty intervals or nested intervals can favor
Polars-only expressions. Older datetime measurements include integer conversion
and do not measure the current direct Date/Datetime support. Comparisons using
joins that would create more than two million intermediate rows were skipped.

Memory depends on grouping too. In separate one- and three-million-row tests,
the function's peak memory for the whole process was 23–30% lower than the
lowest-memory Polars alternative with grouping, but 15–60% higher without it.
These figures include input data and the runtime. See the
[measurement guide](benchmarking.md#memory-metrics) for the exact scope.

<span id="reproduce-and-data"></span>

**Reproduce**

Follow the [release setup](benchmarking.md#setup), then run:

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 1000 100000 1000000 3000000 --dtypes int64 --warmups 3 --repeats 9 --memory --output target/overlap-new.json
```

Use `--help` for workload options and `--compare-plugin PATH` for a saved binary.

See [main samples and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).
[Regenerate tables](benchmarking.md#generate-plots) from saved samples without
running benchmarks.

**History**

Separate runs with different randomly generated data and with one Polars thread
also favored the function on their measured cases. Their samples remain separate
from the main table. The earlier binary (`plugin_reference`) is excluded from
the native comparison.

[Repeat with different random data](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
[one thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json),
and [original join-only evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json)
retain settings and historical comparisons.

</details>
