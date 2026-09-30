# Lane assignment benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`assign_lanes`](api.md#polars_intervals.assign_lanes) places intervals in the
fewest possible lanes so that intervals in the same lane never overlap. In
synthetic benchmarks, its underlying algorithm handled **one million shuffled
intervals in 80–172 ms**. Sorted examples took **24–30 ms**. Other algorithms
were faster on several heavily overlapping inputs. Full Polars query timings
and a native Polars comparison have not been measured.

## Results

**Underlying algorithm time · milliseconds**

--8<-- "docs/assets/benchmarks/lanes-summary.md:3:-3"

Sorted input is much faster to process. On heavily overlapping inputs, other
algorithms can win: the best tested alternative was **3.30× faster** for sorted
nested intervals and **1.79× faster** for shuffled nested intervals.

The repeat run took 59.4 ms for the million-row shuffled, non-overlapping case,
compared with 79.7 ms here. The sorted-nesting result was stable.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and shared
[hardware](benchmarks.md#hardware).

**Measurement and algorithm comparisons**

The headline cases show small and large non-overlapping inputs, plus sorted and
shuffled overlapping inputs where other algorithms can be faster.

The algorithm runs used one thread, two warmups and nine samples per case,
with seed 42 and integer endpoints. The displayed times are medians. Settings,
software versions and source hashes are recorded in the
[environment metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json).

--8<-- "docs/assets/benchmarks/lanes-summary.md:-2:"

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
was stable. Both runs are available below.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

Each run covers 72 combinations of size, input pattern and order, from 1,000 to
one million rows. Patterns include low and high overlap, nested intervals,
duplicates, shared endpoints and empty intervals.

Every timed result is checked for overlaps within lanes and for the minimum
possible lane count. Small cases also use direct pairwise checks. The repeat
checks agreement between the measured implementation and the public Rust
function. Different valid assignments can place rows in different lanes.

Timing includes input checks, sorting, assignment and output construction.
Creating test data and destroying the returned output are excluded. No complete
Polars query, Date/Datetime endpoints or Polars-only alternative was measured.

At one million rows, live algorithm-buffer capacity including output is about
11.4 MiB when few intervals overlap and 27.4 MiB when all overlap. The two-list
alternative uses 19.1 MiB in both cases. These are buffer capacities, not total
process memory. See the [measurement guide](benchmarking.md) and
[algorithm notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#lane-assignment).

<span id="reproduce-and-data"></span>

**Reproduce and data**

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

</details>
