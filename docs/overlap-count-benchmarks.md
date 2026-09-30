# Overlap counting benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`overlap_count`](api.md#polars_intervals.overlap_count) counts how many other
intervals overlap each row. In synthetic benchmarks, full Polars queries processed
**three million shuffled integer intervals in 167–244 ms**, **about 1.4–2.3× faster**
than the fastest native Polars method tested on each example. The advantage
shrinks on small inputs split into many groups, where calling the plugin adds
overhead.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/overlap-summary.md:3:-3"

The plugin's benefit is clearest on the larger inputs. At just 1,000 rows split
across 100 groups, search expressions and the plugin both take about 1 ms.
Those small differences do not establish a dependable winner.

Inputs made entirely of empty or nested intervals can favor native Polars.
Separate memory tests found 23–30% lower whole-process resident memory than the
lowest-memory native alternative with grouping, but 15–60% higher without it.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and native comparisons**

The headline cases show larger inputs with few or many overlaps, plus small
grouped inputs where the runtime difference is inconclusive.

The main run used 24 Polars threads, three warmups and nine samples per case,
with seed 42. The displayed times are medians. Settings, software versions and
source hashes are recorded in the
[main run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).

Sparse inputs have relatively few overlaps. Dense inputs have many. Each group
is processed independently, and the row count is the total across all groups.
All displayed rows use shuffled Int64 endpoints. The native candidates use
search expressions (`native_expr`), parallel search expressions (`native_parallel`),
as-of joins (`native_asof`), an endpoint sweep (`native_sweep`) or an inequality
join (`inequality_join`). The earlier plugin binary (`plugin_reference`) is
excluded from the native comparison.

The summary uses the faster native median for each row. As-of joins win on the
three-million-row ungrouped examples. Search expressions win on the other
displayed examples. Speedup is native median divided by package median.

--8<-- "docs/assets/benchmarks/overlap-summary.md:-2:"

The original detailed table keeps search expressions and as-of joins. All native
candidate samples remain in the linked source data.

--8<-- "docs/assets/benchmarks/overlap-headline.md"

Separate runs with different randomly generated data and with one Polars thread also
favored the plugin on their measured cases. Their samples remain separate from
this table.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

The main run covers 1,000 to three million rows, low and high overlap, sorted
and shuffled input, and one or 100 groups. Results were checked against the
alternative implementations, with additional independent checks on edge cases.
Times include query execution and creation of the result. Data generation and
query construction are excluded. The plugin also validates inputs, while the
Polars-only alternatives assume valid input.

Inputs consisting entirely of empty intervals or nested intervals can favor
Polars-only expressions. Older datetime measurements include integer conversion
and do not measure the current direct Date/Datetime support. Comparisons using
joins that would create more than two million intermediate rows were skipped.

Memory depends on grouping too. In separate one- and three-million-row tests,
the plugin's peak memory for the whole process was 23–30% lower than the
lowest-memory Polars alternative with grouping, but 15–60% higher without it.
These figures include input data and the runtime. See the
[measurement guide](benchmarking.md#memory-metrics) for the exact scope.

<span id="reproduce-and-data"></span>

**Reproduce and data**

Follow the [release setup](benchmarking.md#setup), then run:

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 1000 100000 1000000 3000000 --dtypes int64 --warmups 3 --repeats 9 --memory --output target/overlap-new.json
```

Use `--help` for workload options and `--compare-plugin PATH` for a saved binary.

[Main samples and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json),
[repeat with different random data](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
[one thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json),
and [original join-only evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json)
retain settings and historical comparisons. [Regenerate tables](benchmarking.md#generate-plots)
from these saved samples without running benchmarks.

</details>
