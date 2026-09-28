# Lane assignment benchmarks

## Summary

[`assign_lanes`](usage.md#assign-the-minimum-number-of-lanes) uses a start sort
and min-heap to assign the minimum number of lanes. The heap balances the
measured workloads and uses little working memory at low concurrency; two
sorted streams beat it on many ordered cases, and an endpoint sweep wins some
large shuffled cases.

## Results

**Rust core only · single-threaded · median of 9 samples · i64 ·
[first-run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json)**

--8<-- "docs/assets/benchmarks/lanes-table.md"

The first run times the heap reference implementing the production design;
the repeat separately verifies agreement with the public implementation.

Concurrency and input order both matter. The heap avoids a second index sort,
but maintaining a large active heap can cost more than processing sorted streams
or events. Production is deliberately a compromise: on the sorted nested case,
the two-stream candidate is about 3.3 times faster; on shuffled nesting, the
endpoint sweep is about 1.8 times faster.

The independent repeat supports these tradeoffs, with material run variation:
shuffled disjoint heap time at 1M rows changed from 79.7 to 59.4 ms, and shuffled
concurrency-128 time from 153 to 130 ms. Sorted nested results stayed near
30 ms for the heap and 9 ms for two streams. Both full runs and their
[comparison details](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#repeat-evidence)
are retained; small median differences do not establish a reliable winner.

## Coverage and limitations

Each run contains 72 cases: nine families, sorted/shuffled orders and
1K/10K/100K/1M rows, with two warmups before each nine-sample comparison.
Families include disjoint intervals, fixed concurrency 8 and 128, large cliques,
nesting, staircases, tied endpoints, duplicates and mixed empties. Candidate order
is shuffled, and each candidate receives the same endpoint vectors.

Timing includes validation, allocation, sorting, assignment and row-aligned
output construction. Input generation, correctness checks and returned-output
destruction are outside timing. No complete Polars query, temporal endpoint or
native Polars baseline was measured; these results describe the core choice.

**Peak live buffer capacity**, including output, is approximately 11.4 MiB for
the heap at 1M rows and low concurrency, versus 19.1 MiB for two streams and
49.6 MiB for the endpoint sweep. A million-way clique grows the heap to
27.4 MiB, exceeding the two-stream candidate's 19.1 MiB; the sweep uses 53.6 MiB.
Capacity accounting excludes transient reallocation peaks. It is not an
allocator-instrumented heap or process-RSS measurement. See the shared
[memory definitions and methodology](benchmarking.md).

Validation combines an independent endpoint concurrency sweep with per-lane
non-overlap, shape, contiguous-ID and determinism checks. Every timed result is
checked after timing; small test inputs additionally use a quadratic pairwise
conflict oracle. Production is cross-checked against the measured heap reference
in the repeat. Candidate label identities need not agree, since multiple optimal
assignments can exist. The [design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#lane-assignment)
retain the optimality invariant and detailed fixtures.

## Reproduce and data

<details markdown="1">
<summary>Operation-specific command</summary>

```sh
cargo bench -p intervals-core --bench assign_lanes --locked > target/assign-lanes-local.csv
```

The harness requires an optimized build and writes samples to standard output.
See the shared guide for setup and publishing generated tables.

</details>

[Shared setup and publishing](benchmarking.md) ·
[First-run samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv) ·
[Repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-repeat-windows.csv) ·
[Environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-environment.json) ·
[Algorithm and repeat notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#lane-assignment)
