# Variable-capacity selection benchmarks

[All benchmarks](benchmarks.md)

## Summary

[`max_weight_with_capacity_profile`](api.md#polars_intervals.max_weight_with_capacity_profile)
selects the highest-weight set of intervals when the allowed number of overlaps
changes over time. In synthetic examples with small, independent overlap groups,
full Polars queries took **14.3 ms for 100,000 integer intervals** and **160 ms
for one million**, with the overlap limit varying between 2, 4, 6 and 8.
The result is exact. A native Polars comparison was not measured. Larger,
interconnected groups can take much longer.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/profile-summary.md:3:-3"

Changing the overlap limit adds work on these inputs. Allowing every useful
interval to be selected avoids the harder selection step.

<details markdown="1">
<summary>Benchmark details</summary>

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

The headline table uses integer endpoints to compare constant, changing and
sufficient capacity, including changing-capacity inputs at 100,000 and one million rows.

--8<-- "docs/assets/benchmarks/profile-summary.md:-2:"

<span id="full-polars-measurements"></span>

**Full Polars measurements**

**Complete Polars operation · solver uses up to 8 workers · median of 3 samples ·
Polars thread count unrecorded · [measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json)**

The input consists of separate sets of at most 32 mutually overlapping intervals.
Variable-capacity examples have no available capacity in the gaps between sets.
“Enough capacity for all” allows every useful interval to be selected. Columns
compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/profile-polars-table.md"

A changing capacity profile adds work compared with a constant limit on these
inputs. When capacity is sufficient for every interval, the optimizer can avoid
the harder selection step. A constant profile uses the simpler constant-capacity
solver, with some additional time to read and check the profile.

<span id="underlying-algorithm-comparisons"></span>

**Underlying algorithm comparisons**

**Rust algorithm only · one thread · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json)**

The package chooses between two exact optimization methods. “Transshipment” and
“circulation” are their technical names. This table forces each method on sorted
inputs where capacity alternates between four and five. More profile segments
mean more capacity changes to process. The alternatives include internal timing
instrumentation that the package's direct call does not.

--8<-- "docs/assets/benchmarks/profile-formulations-table.md"

Choosing a suitable method matters much more than small implementation costs
in these examples. The next table compares solving independent overlap sets
one at a time with using eight workers. These are sorted integer inputs.

--8<-- "docs/assets/benchmarks/profile-components-table.md"

The package stays single-threaded below 16,384 rows. Forced parallel processing
loses on the 1,000-row example but wins at 10,000 rows, so that threshold is a
tradeoff. At one million rows, parallel processing is substantially faster.

<span id="coverage-and-limitations"></span>

**Coverage and limitations**

The final algorithm run covers 187 combinations of input size, overlap pattern,
capacity changes, segment count, input order and weights. Large interconnected
selection problems stop at 10,000 intervals. Independent sets and easier cases
extend to one million. The Polars measurements cover integer and temporal
endpoints on the separate-overlap-set inputs described above.

Small cases are checked by trying every subset. Full-sized outputs are checked
against the capacity profile and compared across exact implementations. The
Polars examples have independently calculable best weights. There is no
Polars-only comparison.

Times include input extraction, validation and output construction, excluding
data generation and type conversion. Algorithm-only alternatives have different
input-preparation and instrumentation costs, so small differences need care.
Their times cannot be subtracted from the Polars totals to estimate overhead.

The million-row parallel example requested 143 MB of live heap storage in the
package call. This includes output and working storage, but excludes caller
inputs and the rest of the process. The [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#variable-capacity-selection)
retain memory comparisons, earlier experiments and validation details.

<span id="reproduce-and-data"></span>

**Reproduce and data**

<span id="operation-specific-commands"></span>

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity_profile --locked > benchmarks/results/capacity-profile-local.csv
python -I /path/to/checkout/benchmarks/capacity_profile_temporal.py > capacity-profile-temporal-local.csv
```

`PROFILE_BENCH_MAX_N`, `PROFILE_BENCH_MIN_N`, `PROFILE_BENCH_MIN_M`,
`PROFILE_BENCH_SAMPLES` and `PROFILE_BENCH_FAMILY` restrict the core run. Run the
temporal command with the installed release wheel's Python from outside the
checkout, following the shared setup.

[Setup, metrics and publishing](benchmarking.md) ·
[Final core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-core.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-temporal.csv) ·
[Metadata and historical run inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#variable-capacity-selection)

</details>
