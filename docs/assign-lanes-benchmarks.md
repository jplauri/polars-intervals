# Lane assignment benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`assign_lanes`](api.md#polars_intervals.assign_lanes) places intervals in the
fewest possible lanes so that intervals in the same lane never overlap. In
synthetic benchmarks, full Polars queries handled **one million intervals in
under 180 ms**. Input order and overlap patterns make a large difference.
Polars has no built-in solver for this optimization problem.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/lanes-polars-summary.md:3:-3"

Input already sorted by start is faster to process in these examples. Variable
interval lengths take the longest, while non-overlapping intervals are cheaper.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

The run covers 1,000, 100,000 and one million rows. It compares non-overlapping,
moderately overlapping, nested and variable-length inputs, each sorted by start
or shuffled.

--8<-- "docs/assets/benchmarks/lanes-polars-summary.md:-2:"

**Settings**

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

The run uses package 0.2.0 and Polars 1.44.2. Polars' thread
setting was left at its default, producing a 24-thread pool on this machine.
Each case has two warmups and five timed samples. The table shows seed 7;
seed 41 remains separate in the raw samples. At one million shuffled rows,
the second seed took 59–178 ms, with the same input-pattern tradeoffs.

Inputs have Int64 endpoints, one chunk per column and no groups. Collection
uses the auto engine.

Timing includes public expression and lazy query construction, optimization,
input extraction and validation, sorting, assignment and row-aligned output.
Test-data construction, correctness checks and output destruction are excluded.
Before timing, an independent checker verifies no overlaps within lanes and
the minimum possible lane count. Every timed output is then checked against
that validated deterministic result outside timing.

<span id="coverage-and-limitations"></span>

**Limitations**

Date/Datetime endpoints, grouping, streaming, memory and a native Polars
expression comparison were not measured in this run.

<span id="reproduce-and-data"></span>

**Reproduce**

After the [release rebuild](benchmarking.md#setup), leave `POLARS_MAX_THREADS`
unset and run:

```sh
uv run --no-sync python benchmarks/scheduling_polars.py --output benchmarks/results/scheduling-polars-new
uv run --no-sync python -m unittest discover -s benchmarks -p test_scheduling_polars.py
```

The shared runner measures both lane assignment and weighted scheduling.
Use a new output prefix. For the separate Rust measurements:

```sh
cargo bench -p intervals-core --bench assign_lanes --locked > target/assign-lanes-local.csv
```

The harness requires an optimized build and writes samples to standard output.
See the shared guide for setup and publishing generated tables.

[Shared setup and publishing](benchmarking.md) ·
[First-run samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv) ·
[Repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-repeat-windows.csv) ·
[Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json) ·
[Algorithm and repeat notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#lane-assignment)

Release-library and installed-plugin hashes match for this run.
The [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.metadata.json)
and [measured source archive](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.sources.zip)
preserve the build and all runner inputs.

**History**

These separate Rust measurements include small and large non-overlapping inputs,
plus sorted and shuffled overlapping inputs where private algorithms were faster.
They are not timings of the full Polars calls shown above.

The algorithm runs used one thread, two warmups and nine samples per case,
with seed 42 and integer endpoints. The displayed times are medians. Settings,
software versions and source hashes are recorded in the
[environment metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json).

“Up to 128 overlapping” describes how many intervals can be active at once.
Nested inputs place intervals inside one another. The first method implements
the algorithm used by the package. The alternatives process sorted starts and
ends together, or scan all endpoints in order. They are benchmark comparisons,
not options exposed by `assign_lanes`.

--8<-- "docs/assets/benchmarks/lanes-table.md"

The package's algorithm is fastest on shuffled intervals that do not overlap.
It falls behind when many intervals overlap. For sorted nested input, the
two-list alternative takes 9.08 ms compared with 29.9 ms. For shuffled nested
input, the endpoint-scan alternative takes 96 ms compared with 172 ms.

An independent repeat showed the same tradeoffs, with some runtime variation.
The million-row shuffled, non-overlapping case took 59.4 ms on that repeat,
compared with 79.7 ms here. The roughly 30 ms versus 9 ms sorted-nesting result
was stable. Both runs are linked under Reproduce.

Each run covers 72 combinations of size, input pattern and order, from 1,000 to
one million rows. Patterns include low and high overlap, nested intervals,
duplicates, shared endpoints and empty intervals.

Every timed result is checked for overlaps within lanes and for the minimum
possible lane count. Small cases also use direct pairwise checks. The repeat
checks agreement between the measured implementation and the public Rust
function. Different valid assignments can place rows in different lanes.

Timing includes input checks, sorting, assignment and output construction.
Creating test data and destroying the returned output are excluded. These
historical runs measured no complete Polars query, Date/Datetime endpoints
or Polars-only alternative.

At one million rows, live algorithm-buffer capacity including output is about
11.4 MiB when few intervals overlap and 27.4 MiB when all overlap. The two-list
alternative uses 19.1 MiB in both cases. These are buffer capacities, not total
process memory. See the [measurement guide](benchmarking.md) and
[algorithm notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#lane-assignment).

</details>
