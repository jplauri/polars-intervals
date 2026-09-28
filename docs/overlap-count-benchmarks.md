# Overlap counting benchmarks

## Summary

The production [overlap counter](api.md#polars_intervals.overlap_count) uses an
endpoint sweep without materializing interval pairs. It is faster than the
native counting formulations throughout the recorded 100K–3M-row matrix, while
native expressions can win with tiny groups. Grouping also changes the memory
tradeoff: the plugin's process peak is lower in the measured grouped cases but
higher in the ungrouped cases.

## Results

Polars collection · 24 threads · median of 9 samples ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json)

--8<-- "docs/assets/benchmarks/overlap-headline.md"

These are shuffled Int64 inputs from one run. Search sorted and As-of joins are
two individually named native baselines, not a per-row choice of the fastest
method. The raw report also contains a parallel search layout, a native endpoint
sweep, a bounded inequality join, and an older plugin binary.

The larger workloads favor production; a thousand rows spread over a hundred
groups expose its fixed overhead. The small median differences there do not
establish a reliable winner. A separate second-seed run with 10/1,000 groups and
datetime casts favored the plugin in all 24 cases; a separate one-thread run
favored it in all 16 cases. These runs support the trend without being pooled
into this table.

## Coverage and limitations

The main matrix spans 1K–3M rows, sparse/dense overlap, shuffled/start-sorted
order, and one or 100 balanced groups. Sparse starts are four units apart with
lengths 0–8; dense starts and lengths occupy roughly a thousand units. The
production path validates inputs; native expressions assume valid, non-null
data. Complete `collect(engine="in-memory")` calls on prebuilt plans are timed,
including output materialization. Independent edge-case checks and per-case
output comparisons, including dtype and row order, run outside timing; see the
[runner](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py)
and [shared methodology](benchmarking.md#measurement-rules).

Historical datetime measurements include explicit integer casts, so they do not
measure today's direct Date/Datetime adapter. All-empty and nested inputs are
also known native-favorable cases, without a headline timing matrix here. The
inequality join is skipped above two million candidate rows; grouped joins count
equality-join candidates before overlap filtering.

At 1M/3M rows, production's **total peak process RSS** was 23–30% lower than the
smallest native peak for grouped cases, but 15–60% higher globally. These are
separate cold processes with inputs loaded; the query's RSS increase is a
high-water-mark change, not an allocation count. See [memory
definitions](benchmarking.md#memory-metrics).

## Reproduce and data

<details markdown="1">
<summary>Reproduce overlap counting</summary>

Follow the [release setup](benchmarking.md#setup), then run:

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 1000 100000 1000000 3000000 --dtypes int64 --warmups 3 --repeats 9 --memory --output target/overlap-new.json
```

Use `--help` for workload options and `--compare-plugin PATH` for a saved binary.

</details>

[Main samples and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json),
[second seed](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
[one thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json),
and [original join-only evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json)
retain settings and historical comparisons. [Regenerate tables](benchmarking.md#generate-plots)
from these saved samples without running benchmarks.
