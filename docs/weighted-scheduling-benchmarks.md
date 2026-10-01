# Weighted scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`max_weight_non_overlapping`](api.md#polars_intervals.max_weight_non_overlapping)
selects non-overlapping intervals with the highest possible total weight. In
synthetic benchmarks with positive weights, full Polars queries processed **one
million intervals in under 270 ms**. Input already sorted by start was
faster. Variable interval lengths cost more than the structured examples. Polars
has no built-in solver for this optimization problem.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/weighted-polars-summary.md:3:-3"

Shuffled intervals with variable lengths take about a quarter of a second at
one million rows. The other shuffled examples finish in about a tenth of a second.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The run covers 1,000, 100,000 and one million rows. It compares non-overlapping,
moderately overlapping, nested and variable-length inputs, each sorted by start
or shuffled.

--8<-- "docs/assets/benchmarks/weighted-polars-summary.md:-2:"

**Settings**

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

The run uses package 0.2.0 and Polars 1.44.2. Polars' thread
setting was left at its default, producing a 24-thread pool on this machine.
Each case has two warmups and five timed samples. The table shows seed 7;
seed 41 remains separate in the raw samples. Its million-row shuffled medians
were 96–260 ms, with the same input-pattern tradeoffs.

Inputs have Int64 endpoints and positive Int64 weights, one chunk per column
and no groups. Collection uses the auto engine.

Timing includes public expression and lazy query construction, optimization,
input extraction and validation, sorting, selection and row-aligned output.
Test-data construction, correctness checks and output destruction are excluded.
Before timing, each output is checked for feasibility and optimal total weight
using an independent start-ordered suffix dynamic program. The checker is
tested against every subset on small problems. Every timed output is then
compared with that validated deterministic result outside timing.

<span id="coverage-and-limitations"></span>

**Limitations**

Date/Datetime endpoints, other weight distributions, grouping, streaming,
memory and a native Polars expression comparison were not measured in this run.

<span id="reproduce-and-data"></span>

**Reproduce**

After the [release rebuild](benchmarking.md#setup), leave `POLARS_MAX_THREADS`
unset and run:

```sh
uv run --no-sync python benchmarks/scheduling_polars.py --output benchmarks/results/scheduling-polars-new
uv run --no-sync python -m unittest discover -s benchmarks -p test_scheduling_polars.py
```

The shared runner measures both scheduling operations. Use a new output prefix.
For the separate Rust measurements:

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

<span id="historical-validation"></span>

Release-library and installed-plugin hashes match for this run.
The [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.metadata.json)
and [measured source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.sources.zip)
preserve the build and all runner inputs.

**History**

These separate Rust measurements show small and large non-overlapping inputs,
gains on structured inputs, and losses on sorted or shuffled random-length inputs.
They are not timings of the full Polars calls shown above.

The algorithm runs used one thread, two warmups and five samples per case,
with seed 42 and integer endpoints and weights. The displayed times are medians.
Settings, software versions and source hashes are recorded in the
[environment metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json).

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

The tests cover 1,000 to one million rows, eleven interval patterns, sorted and
shuffled input, and six weight distributions. They include nesting, duplicates,
shared endpoints, empty intervals, negative or zero weights, and a single
high-value interval competing with many smaller ones.

Every timed result is checked for non-overlap and optimal total weight against
an independent implementation. Small tests also try every possible subset.
Several different selections can have the same optimal weight.

These are comparisons of the underlying algorithms, including input validation,
sorting and result construction. They include internal measurement overhead.
These historical runs measured no complete Polars queries, Date/Datetime
endpoints or Polars-only comparisons.

At one million positive, nonempty intervals, the chosen design and binary-search
alternative both use 39.1 MiB of live buffer capacity. The endpoint-scan
alternative uses 77.2 MiB. This measures algorithm buffers rather than the
whole process. See the [measurement guide](benchmarking.md) and
[supporting comparisons and checks](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#weighted-scheduling).

</details>
