# Overlap counting benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`overlap_count`](api.md#polars_intervals.overlap_count) counts how many other
intervals overlap each row. In the displayed benchmarks, complete Polars queries
processed **three million shuffled integer intervals in 167–244 ms**. The two
Polars-only alternatives took 339–764 ms on those same examples. The advantage
shrinks on small inputs split into many groups, where calling the plugin adds
overhead.

## Results

**Complete Polars queries · 24 Polars threads · median of 9 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json)**

Sparse inputs have relatively few overlaps. Dense inputs have many. Each group
is processed independently, and the row count is the total across all groups.
All rows below use shuffled Int64 endpoints. The alternatives implement the
same counting operation using Polars search or join expressions.

--8<-- "docs/assets/benchmarks/overlap-headline.md"

The plugin's benefit is clearest on the larger inputs. At just 1,000 rows split
across 100 groups, search expressions and the plugin both take about 1 ms.
Those small differences do not establish a dependable winner.

Separate runs with different randomly generated data and with one Polars thread also
favored the plugin on their measured cases. Their samples remain separate from
this table.

## Coverage and limitations

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
[repeat with different random data](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
[one thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json),
and [original join-only evidence](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json)
retain settings and historical comparisons. [Regenerate tables](benchmarking.md#generate-plots)
from these saved samples without running benchmarks.
