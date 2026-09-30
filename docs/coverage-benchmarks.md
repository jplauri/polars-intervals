# Maximum covered length benchmarks { #maximum-k-coverage-benchmarks }

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_k_coverage`](api.md#polars_intervals.max_k_coverage) selects at most `k`
intervals to cover the greatest total length, counting overlaps only once. The
result is exact. On a synthetic input of **one million shuffled integer
intervals**, a complete Polars query took **161 ms when allowed to select 8
intervals** and **681 ms when allowed to select 64**, using the same overlapping
input pattern. Allowing more selected intervals can increase both runtime and
working memory. Polars has no built-in solver for this optimization problem.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/coverage-summary.md:3:-3"

On these integer inputs, increasing the selection limit from 8 to 64 makes
the query about four times slower, while sorting helps at the smaller limit.
These overlapping-step examples show a measured tradeoff, not a fixed scaling
rule for every input.

<details markdown="1">
<summary>Benchmark details</summary>

**What was compared**

--8<-- "docs/assets/benchmarks/coverage-summary.md:-2:"

The compact table selects integer overlapping-step inputs: sorted cases at
10,000 and one million rows with a selection limit of 8, and shuffled
million-row cases with limits of 8 and 64.

The inputs form overlapping steps along the coordinate range. The selection
limit is the most intervals the function may choose. Columns compare integer,
Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/coverage-collection-table.md"

On one million integer rows, increasing the limit from 8 to 64 makes this
example about four times slower. Sorting helps at the smaller limit: the
sorted example takes 125 ms compared with 161 ms when shuffled. These are
measured cases, not a fixed scaling rule for every input.

**Settings**

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

**Complete Polars queries · 24 Polars threads · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json)**

Small tests try every possible subset. A separate slower optimizer checks
larger test cases. Benchmark selections are independently combined to verify
their covered length and compared with reference answers. The Polars examples
have known best results.

Query times include planning, execution and retrieving the selection. Creating
and converting inputs is excluded.

<span id="coverage-and-limitations"></span>

**Limitations**

The algorithm run covers 1,506 combinations of size, interval pattern, input
order and selection limit. It spans 1,000 to one million rows and limits from
zero to 64, with extra tests for limits larger than the input. Only a subset
of patterns and limits reaches one million rows.

Polars performance coverage is narrower: three input patterns, sorted and
shuffled orders, and integer, Date and microsecond Datetime endpoints.
Grouping and other datetime variants have no matching timings here.
No equivalent Polars-only optimizer was measured.

Some slower algorithm comparisons were limited to smaller datasets. The
[supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#maximum-k-coverage)
retain these limits, memory tradeoffs and correctness checks.

<span id="reproduce-and-data"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench max_k_coverage --locked > target/coverage-core.csv
# From an external environment with the release wheel installed:
python -I /path/to/checkout/benchmarks/coverage_temporal.py > coverage-temporal.csv
```

The core harness performs separate correctness and allocation calls before
three timed invocations. See the shared guide for release setup.

[Shared setup and publishing](benchmarking.md) ·
[Core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-core.csv) ·
[Collection samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv) ·
[Metadata, source hashes and validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json) ·
[Recurrence, proofs and experiments](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/algorithm-notes.md#maximum-k-coverage)

**History**

**Rust algorithm only · one thread · median of 3 samples · integer endpoints ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-environment.json)**

The package avoids work on intervals that cannot improve the answer. One
reference uses a simpler version of the same optimization method. Another
solves independent overlap sets separately. The references include internal
timing instrumentation and use different input wrappers.

--8<-- "docs/assets/benchmarks/coverage-core-table.md"

Solving independent sets separately wins at the larger selection limit in this
example, but adds overhead at the smaller limit. It is not a mode exposed by
the package. When the limit is sufficient to cover the entire input range,
the package can skip much of the optimization work.

On the million-row overlapping-step input, the package's peak requested heap
storage rises from 162 MiB at a limit of 8 to 216 MiB at 64. These are
algorithm-only allocations, including output and working storage. They do not
measure the memory of a complete Polars query.

</details>
