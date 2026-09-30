# Weighted scheduling benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`max_weight_non_overlapping`](api.md#polars_intervals.max_weight_non_overlapping)
selects non-overlapping intervals with the highest possible total weight. In
synthetic benchmarks, its underlying algorithm processed **one million intervals
in 21–240 ms**. Input order and interval lengths make a large difference.
Alternative algorithms were faster on random-length examples. Full Polars query
timings and a native Polars comparison have not been measured.

## Results

**Underlying algorithm time · milliseconds**

--8<-- "docs/assets/benchmarks/weighted-summary.md:3:-3"

The package's algorithm works well on non-overlapping and nested inputs.
Random lengths reverse the advantage: the best tested alternative was
**1.57× faster** on the shuffled example. A separate repeat confirmed these
gains and losses.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and algorithm comparisons**

The headline cases show small and large non-overlapping inputs, gains on
structured inputs, and losses on sorted or shuffled random-length inputs.

The algorithm runs used one thread, two warmups and five samples per case,
with seed 42 and integer endpoints and weights. The displayed times are medians.
Settings, software versions and source hashes are recorded in the
[environment metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json).

--8<-- "docs/assets/benchmarks/weighted-summary.md:-2:"

All weights in this table are positive. “Sorted by end” means the input is
ordered by interval finish time. The first method implements the algorithm
chosen for the package. The alternatives find compatible intervals using binary
search or an ordered scan of endpoints. They are benchmark implementations,
not selectable modes of the public function.

--8<-- "docs/assets/benchmarks/weighted-table.md"

The chosen algorithm works well on non-overlapping and nested inputs. It takes
81.9 ms for one million shuffled, non-overlapping rows, compared with 148 ms for
binary search. Random interval lengths reverse the result: on the shuffled
example, the endpoint-scan alternative takes 153 ms compared with 240 ms.

An independent repeat confirmed both the gains on structured inputs and the
losses on random lengths. Its samples remain separate, so the table describes
one run rather than an average across machines or runs.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

The tests cover 1,000 to one million rows, eleven interval patterns, sorted and
shuffled input, and six weight distributions. They include nesting, duplicates,
shared endpoints, empty intervals, negative or zero weights, and a single
high-value interval competing with many smaller ones.

Every timed result is checked for non-overlap and optimal total weight against
an independent implementation. Small tests also try every possible subset.
Several different selections can have the same optimal weight.

These are comparisons of the underlying algorithms, including input validation,
sorting and result construction. They include internal measurement overhead.
There are no complete Polars-query timings, Date/Datetime measurements or
Polars-only comparisons in this report.

At one million positive, nonempty intervals, the chosen design and binary-search
alternative both use 39.1 MiB of live buffer capacity. The endpoint-scan
alternative uses 77.2 MiB. This measures algorithm buffers rather than the
whole process. See the [measurement guide](benchmarking.md) and
[supporting comparisons and checks](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling).

<span id="reproduce-and-data"></span>

**Reproduce and data**

<span id="historical-validation"></span>

```sh
cargo bench -p intervals-core --bench max_weight_non_overlapping --locked > target/weighted-local.csv
```

Set `WEIGHTED_BENCH_MAX_N=1000` for a smoke run. The core harness requires release
mode and does not need Polars. Historical package-check results moved to the
[validation record](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#repeat-and-historical-validation).

[Shared setup and publishing](benchmarking.md) ·
[First-run samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv) ·
[Repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-repeat-windows.csv) ·
[Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json) ·
[Algorithm notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling)

</details>
