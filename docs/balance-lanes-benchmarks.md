# Lane balancing benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`assign_balanced_lanes`](api.md#polars_intervals.assign_balanced_lanes) places
intervals in the fewest possible lanes without overlaps within a lane, then
tries to distribute their row counts evenly. On three synthetic **100,000-row
inputs, complete Polars queries took 3.76–24.1 ms**, versus 0.91–1.39 ms for
ordinary assignment. In a separate suite, it improved balance in **108 of 132
datasets**. It never worsens its starting balance but does not guarantee the
best balance. No equivalent native Polars optimizer was measured.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/balance-summary.md:3:-3"

On the eight-lane input, balancing reduced the gap between the fullest and
emptiest lanes from **99,992 rows to zero**, giving 12,500 rows in each lane.
The two long-interval examples stay uneven because their overlaps force it,
so the extra search time brings no improvement.

Ordinary assignment minimizes the number of lanes without trying to balance
their row counts. It is a cost baseline for a different objective.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

--8<-- "docs/assets/benchmarks/balance-summary.md:-2:"

The compact table selects a small input with empty intervals, an input that
can balance evenly across eight lanes, and two long/short-interval inputs
whose overlaps force uneven lane sizes.

**Full Polars measurements**

**Complete Polars queries · one thread · median of 5 samples after warmup ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.metadata.json)**

Ordinary assignment uses `assign_lanes`. New balanced assignment uses
`assign_balanced_lanes` with its default settings. Improving existing lanes uses
the same function with `initial_lanes` supplied. Its times exclude creating the
original assignment.

--8<-- "docs/assets/benchmarks/balance-polars-table.md"

In the eight-lane example, ordinary assignment put 99,993 rows in one lane and
one row in each of the others. New balanced assignment produced **12,500 rows in
every lane**, taking 3.76 ms instead of 0.91 ms. Improving the existing assignment
did not achieve that improvement with the default search limit.

The table below shows the difference between the largest and smallest lane's
row counts on the same inputs. Smaller is better, and zero means equal counts.

| Input | Ordinary assignment | Improve existing lanes | New balanced assignment |
| --- | ---: | ---: | ---: |
| 11 rows, including 9 empty intervals | 9 | 1 | 1 |
| 100,000 rows needing 8 lanes | 99,992 | 99,992 | 0 |
| One long interval overlapping 99,999 short ones | 99,998 | 99,998 | 99,998 |
| 50,000 long intervals overlapping 50,000 short ones | 49,999 | 49,999 | 49,999 |

The last two examples show the cost of searching without improving balance.
Their overlaps force uneven lane sizes when using the minimum number of lanes.
In the last example, balanced assignment takes 24.1 ms compared with 1.39 ms.
The [saved quality results](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.quality.csv)
include the lane counts behind these comparisons.

**Algorithm-only timings and search settings**

**Rust algorithm only · median of 5 samples after two warmups ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-core-windows-20260928.metadata.json)**

“Initial choices only” selects the best starting assignment without further
search. The other columns correspond to the user-facing operations above.
These inputs differ from the Polars examples.

--8<-- "docs/assets/benchmarks/balance-core-table.md"

The default `max_work=100_000` limits search effort, not elapsed milliseconds.
Across 60 algorithm test cases, increasing it from 10,000 to 100,000 improved
balance in four cases. Increasing it to one million improved two more.
More search can help, but does not guarantee improvement. Input checks and
preparation still take time outside that search limit.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

Balance measures numbers of rows, including empty intervals. It does not measure
the total duration of intervals in each lane. Every output is checked for
overlaps within lanes and for the minimum lane count.

Of the 132 datasets in the separate size-scaling suite, 24 were constructed with
a known best balance. New balanced assignment reached it in all 24. Improving
ordinary assignments reached it in 12. The best possible balance is unknown for
the remaining 108 datasets, so improvement alone does not establish optimality.

Other tests cover small datasets whose assignments can be checked exhaustively,
Date/Datetime endpoints, groups and streaming queries. A supplied three-lane
example remains at counts 15, 16 and 17 even though 16 in every lane is possible.
This demonstrates a real limit of the search. Million-row performance and other
machines were not measured here.

The saved runs use older names for the same construction and improvement modes.
The [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/balance-lanes-notes.md)
retain their mapping, detailed quality checks and measurement boundaries.

<span id="reproduce-and-data"></span>

**Reproduce and data**

Follow the [release-plugin setup](benchmarking.md#setup). Generate the existing
corpora first, then run the complete commands in the
[supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/balance-lanes-notes.md#reproduce-the-recorded-matrix).
A small offline run is:

```sh
python benchmarks/generate_interval_graphs.py synthetic --suite smoke --seed 42 --output target/balance-smoke
uv run --no-sync python benchmarks/balance_lanes.py --dataset target/balance-smoke --handcrafted --max-work 100000 --warmups 1 --repeats 5 --output benchmarks/results/balance-local-new
cargo bench -p intervals-core --bench balance_lanes --locked
```

Set `POLARS_MAX_THREADS=1` before Python. Use fresh output prefixes and run
benchmarks one at a time after compilation. The runner saves the build version,
settings and dataset identifiers. The supporting notes list the settings needed
to reproduce the complete run.

Saved [quality](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.summary.json),
[scaling](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-scaling-windows-20260928.summary.json)
and [execution-variant](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-workloads-windows-20260928.metadata.json)
records link raw samples, provenance and source hashes. The
[verification log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-verification-windows-20260928.md)
records executed checks. Generated datasets stay outside version control.
Table downloads retain sample ranges and exact medians.

</details>
