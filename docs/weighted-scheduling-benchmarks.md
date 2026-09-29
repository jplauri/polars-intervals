# Weighted scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_weight_non_overlapping`](api.md#polars_intervals.max_weight_non_overlapping)
selects non-overlapping intervals with the highest possible total weight. In
algorithm-only benchmarks, the design used by the package processed **one million
intervals in 21–240 ms** across the displayed examples. Input order and interval
lengths make a large difference. These times exclude Polars overhead, and
alternative algorithms were faster on the random-length examples.

## Results

**Rust algorithm only · one thread · median of 5 samples · integer endpoints
and weights · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json)**

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

## Coverage and limitations

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

## Reproduce and data

<details markdown="1" id="historical-validation">
<summary>Operation-specific command and historical validation</summary>

```sh
cargo bench -p intervals-core --bench max_weight_non_overlapping --locked > target/weighted-local.csv
```

Set `WEIGHTED_BENCH_MAX_N=1000` for a smoke run. The core harness requires release
mode and does not need Polars. Historical package-check results moved to the
[validation record](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#repeat-and-historical-validation).

</details>

[Shared setup and publishing](benchmarking.md) ·
[First-run samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv) ·
[Repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-repeat-windows.csv) ·
[Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json) ·
[Algorithm notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling)
