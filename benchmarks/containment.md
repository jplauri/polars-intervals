# Containment counting: design and measurements

`containment_count` counts other rows satisfying `start_j >= start_i` and
`end_j <= end_i`. It is offline two-dimensional dominance counting. Empty
intervals participate in exactly the same endpoint predicate; duplicates count
one another, and each row excludes itself once.

## Candidates and correctness invariant

The release-mode Rust harness implements three exact candidates:

| Candidate | Start ordering | End counter |
| --- | --- | --- |
| A: packed Fenwick | `(start, end, original_index)` records | Fenwick prefix sum |
| B: indirect Fenwick | indices into the original endpoint arrays | Fenwick prefix sum |
| C: packed segment tree | the same records as A | iterative segment-tree prefix sum |

All compress end coordinates, sort starts descending with end/index tie-breaks,
and insert **every** row in an equal-start group before querying any row in
that group. The tree then represents exactly `start_j >= start_i`. An inclusive
end prefix imposes `end_j <= end_i`; subtracting one excludes self. Equal ends
share one compressed coordinate and retain their multiplicity. Ranks temporarily
occupy the output buffer and are replaced with counts after the group's updates.

For example, `[1,5), [1,5), [1,10)` must produce `[1,1,2]` in this order.
Sorting ties and querying rows before the whole group has been inserted would
undercount. `[0,5), [5,5)` produces `[1,0]`, despite the half-open representation.

Only A is linked into the production library. B and C remain under `benches/`
for reproduction and independent cross-checking. We did not add CDQ or radix
sorting: the three required candidates provide a clear measured choice without
another algorithm or dependency. There is no custom sorted-input fast path;
the measured implementation uses Rust's adaptive `sort_unstable` throughout.

All three take `O(n log n)` time and `O(n)` auxiliary memory. Endpoints use only
comparisons and copies, preserving signed/unsigned extremes without subtraction
or negation. Counts use `usize` in core, bounded by the collection size, and
become non-null `UInt64` at the Polars boundary. Core has zero production
dependencies, including no Polars, Arrow, Python, chrono or PyO3 dependency.

## Datasets and timing protocol

The Rust harness covers all 14 structures at 1K, 10K, 100K, 1M and 3M rows:
disjoint, strict nesting, complete duplicates, equal starts, equal ends, random
sparse, random dense, broad outers around short intervals, crossing antichains,
many empties, mixed empty/non-empty, descending-start sorted, reverse-sorted and
shuffled nesting. Randomness has fixed seeds. Rust uses a portable xorshift;
Python uses `random.Random`, so their random datasets are reproducible but not
identical. The last three Rust cases isolate input-order effects on the same
nesting family.

Before timing, the harness checks all candidates against an independent nested
loop oracle on small data and against production on every full-sized case.
Five raw samples per candidate alternate execution order, after full-output
verification/warmup. Each result is also checked outside its timed region.
Total kernel time includes validation, allocation, compression, sorting and
sweep; phase columns separately time compression (including rank lookup), record
sorting, and tree allocation/updates/queries. Input generation and destruction
of the returned output are excluded. Phase clock overhead is included in totals.

Polars timings include expression execution, extraction, kernel and result
construction, and any native preprocessing/restoration of row order. Queries
are built outside timing, collected on the in-memory engine, warmed once and
measured five times with alternating execution order. Every timed output must
match. Small structural and seeded random cases also check every native
formulation directly against an independent Python oracle.
The native queries assume valid, matching endpoints; the plugin's timed work
also includes its public input validation.

Grouped cases use 10K/100K total rows, 1/10/100/1000 interleaved groups and sparse,
dense and mixed structures. Even the one-group window case explicitly uses
`.over("group")`. Date and UTC Datetime(us) are sampled at 10K/100K, globally
and with 100 groups; Date uses i32, Datetime uses the same i64 kernel as Int64.
The Python suite tests all three Datetime units and timezone metadata separately.

## Native Polars comparisons

The self-join uses `join_where(start <= start_right, end_right <= end)` and
aggregates by original row id, subtracting one and restoring row order.
Every valid row matches itself, including empties, so a fill/join-back and an
extra identity inequality are unnecessary. This excludes exactly the same row,
while keeping duplicate identities distinct.

The safety cap is **2,000,000 intermediate rows**, including self. Before any
join executes, the verified plugin result gives the exact match count
`sum(counts) + n`. Unsafe cases are skipped and recorded with their row count;
they are never attempted merely to demonstrate a memory failure.

For grouped joins we compare an equality predicate plus the two inequalities,
and a native coordinate-band encoding that separates groups before IEJoin.
The equality plan's conservative safety bound is `sum(group_size ** 2)`.
The band variant includes rank/offset preprocessing in the timed lazy query;
its integer offsets are safe for these bounded benchmark datasets and are not
offered as a general arbitrary-endpoint API. Optimized plans are stored in the
raw report so the optimizer's choices can be reviewed.

There **is** a clean native non-pair-materializing alternative in Polars 1.44.2:

1. Save row ids and sort by descending start, then ascending end.
2. Compute the end's maximum rank in its expanding prefix, using
   `rolling_rank(window_size=n, min_samples=1, method="max")`.
3. Take the maximum rank over each identical `(start, end)` family, subtract
   one, and restore row order. Grouped versions include the group in each step.

Earlier equal-start rows with smaller ends already qualify; later larger ends
do not. Later exact duplicates are the only qualifying rows missing from a
prefix, and the family's maximum fixes them. This avoids materializing pairs.
We also benchmark `rolling_rank_by` on a sorted position column, with an integer
window wide enough to include the entire prefix. It avoids the fixed-window
counter's up-front allocation for the full window length in small groups.

The [rolling rank documentation](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.rolling_rank.html)
defines the trailing-value rank semantics. The inspected Polars 0.55.2 Rust
implementation (`polars-compute/src/rolling/no_nulls/rank.rs` and
`polars-utils/src/order_statistic_tree.rs`) uses a weight-balanced order-statistic
tree: insertion and rank lookup take logarithmic time. Thus this native query
also takes `O(n log n)` time with linear live data, rather than quadratic pairs.
These rank APIs are currently marked unstable. The benchmark does not claim
that a plugin is necessary to obtain the asymptotic improvement.

We also investigated a leaner `rank_runs` formulation using
[`rolling_rank_by`](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.rolling_rank_by.html).
Sort by descending start
only, then pass `start.rle_id()` as the dynamic rolling rank's `by` coordinate.
These bounded, increasing run ids represent equal-start groups without negating
signed/unsigned extremes. A closed window of width n includes the complete
current start group and every earlier group. Ranking the current end directly
therefore gives the inclusive dominance count; subtract one and restore row ids.
This removes the end sort and duplicate-family correction. The benchmark
implements and oracle-checks all three native rank formulations, including
group windows. An input row count or safe upper bound supplies the window width.

The original native comparison and this subsequent `rank_runs` investigation
are saved as separate runs with their own samples and source hashes. The latter
also remeasures the plugin, so their within-run comparison has the same conditions.

See also the [native join API](https://docs.pola.rs/api/python/stable/reference/lazyframe/api/polars.LazyFrame.join_where.html).

## Measured results

Recorded September 26, 2026 on Windows 11, AMD Ryzen 9 3900X (12 cores /
24 logical CPUs), rustc 1.98.1, Python 3.14.0, Polars 1.44.2
with 24 Polars threads. Kernels are single-threaded; Polars may parallelize
sorting and groups. These are five-sample medians on one machine, not universal
speed guarantees. Both benchmark suites ran sequentially after builds/checks.

Source hashes describe the code at measurement time. Subsequent cleanup
simplified dtype dispatch and removed duplicate untimed benchmark checks;
the measured kernels and native query definitions are unchanged.
Integration with the current shared Polars adapter subsequently replaced
unconditional endpoint copies with borrowing contiguous columns. The archived
Polars timings describe the original copying adapter, not a remeasurement of
that integration. Reference candidates moved from `benches/support/mod.rs` to
`benches/support/containment.rs` without algorithm changes.

Raw data and provenance:

- [Kernel CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv)
- [Kernel environment and source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.json)
- [Polars samples, plans, skips and environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-windows.json)
- [Follow-up run-rank samples and environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json)

### Kernel medians at 3M rows (ms)

| Structure | A: packed Fenwick | B: indirect Fenwick | C: segment tree |
| --- | ---: | ---: | ---: |
| disjoint | 274.7 | 255.4 | 371.6 |
| nested | 274.7 | 262.0 | 358.5 |
| duplicates | 53.2 | 38.6 | 53.8 |
| equal_starts | 193.8 | 186.2 | 358.6 |
| equal_ends | 53.0 | 38.1 | 54.2 |
| sparse | 1006.3 | 1344.0 | 1117.1 |
| dense | 418.4 | 779.2 | 487.7 |
| broad | 447.5 | 457.5 | 533.5 |
| crossing | 272.9 | 255.5 | 362.7 |
| empty | 295.1 | 753.3 | 302.7 |
| mixed | 362.9 | 862.9 | 365.9 |
| sorted | 272.1 | 260.4 | 356.7 |
| reverse | 278.1 | 260.9 | 357.2 |
| shuffled | 1049.4 | 1385.8 | 1248.1 |

### Production choice

**A, packed records plus Fenwick, is the production implementation.** Its
contiguous endpoint comparisons substantially reduce sorting cost on random
and tie-heavy inputs. B saves record-copying and memory and wins several
already ordered / low-cardinality cases. We accept that tradeoff for the
larger gains on shuffled, sparse, dense and mixed workloads. C provides an
independent exact counter but generally pays more for updates/queries and
memory; isolated wins do not justify a production dispatcher.

Across the 70 equally weighted size/structure cells, the geometric mean of
A/B runtime ratios is 1.012 (less than one favors A).
This weighting is descriptive of this suite, not a prediction of user traffic.
In particular, the normalized all-size score does not favor A. We choose it for
lower absolute cost on the larger irregular workloads, rather than claiming it
wins a majority of cells. The indirect candidate remains useful for reproducing
the smaller-memory/ordered-input tradeoff; we do not add a runtime backend switch.
Summing one median execution of every size/structure cell gives 6.53 s for A,
8.63 s for B, and 8.16 s for C. That aggregate is dominated by the large cases;
it explains the throughput choice without erasing B's wins on smaller inputs.

### Selected A phase medians at 3M rows (ms)

| Structure | Compression/ranks | Record sort | Counter sweep | Total |
| --- | ---: | ---: | ---: | ---: |
| disjoint | 101.2 | 10.6 | 140.9 | 274.7 |
| dense | 64.6 | 211.9 | 118.4 | 418.4 |
| sparse | 540.6 | 133.8 | 285.3 | 1006.3 |
| mixed | 42.7 | 200.5 | 86.4 | 362.9 |
| shuffled | 617.4 | 133.4 | 275.9 | 1049.4 |

Phase medians need not sum to the total; validation/record construction and
other allocations are included only in total time.

### Native Polars: global Int64, 1M rows (ms)

| Structure | Plugin | Native rank | Native rank_by | IEJoin |
| --- | ---: | ---: | ---: | ---: |
| disjoint | 73.33 | 882.01 | 915.18 | 259.37 |
| nested | 77.25 | 1049.46 | 1106.05 | skip |
| duplicates | 22.24 | 61.70 | 79.39 | skip |
| equal_starts | 65.42 | 1019.29 | 1072.84 | skip |
| equal_ends | 22.10 | 88.26 | 96.75 | skip |
| sparse | 218.66 | 654.90 | 660.79 | 357.03 |
| dense | 114.50 | 325.62 | 349.82 | skip |
| broad | 123.98 | 878.61 | 916.01 | skip |
| crossing | 75.82 | 876.79 | 919.76 | 247.67 |
| empty | 85.03 | 179.48 | 205.74 | skip |
| mixed | 97.51 | 169.35 | 184.08 | skip |
| sorted | 76.64 | 1034.94 | 1081.08 | skip |
| reverse | 74.92 | 1013.09 | 1060.68 | skip |
| shuffled | 187.18 | 1054.38 | 1092.60 | skip |

All missing join times are safety skips. At 3M rows even disjoint data
exceeds the 2M cap once self-matches are included; the native rank methods
remain in the comparison at that size.

### Leaner native run-rank: global Int64, 3M rows (ms)

This follow-up includes fresh plugin measurements alongside `rank_runs`.
Compare within each row; the earlier prefix-rank tables come from a separate run.

| Structure | Plugin | Native rank_runs |
| --- | ---: | ---: |
| disjoint | 274.17 | 2792.37 |
| nested | 282.04 | 3332.80 |
| duplicates | 65.34 | 146.47 |
| equal_starts | 202.13 | 2756.34 |
| equal_ends | 66.52 | 187.65 |
| sparse | 1089.34 | 2012.46 |
| dense | 410.76 | 1338.39 |
| broad | 450.99 | 2778.05 |
| crossing | 275.11 | 2733.39 |
| empty | 307.15 | 544.18 |
| mixed | 362.61 | 458.66 |
| sorted | 278.45 | 3251.54 |
| reverse | 291.59 | 3297.83 |
| shuffled | 1101.40 | 3304.81 |

### Pair-materializing joins at 1K rows (ms)

| Structure | Rows materialized (including self) | Plugin | IEJoin |
| --- | ---: | ---: | ---: |
| nested | 500,500 | 0.23 | 5.42 |
| duplicates | 1,000,000 | 0.30 | 9.03 |
| sparse | 1,467 | 0.29 | 1.83 |
| dense | 128,045 | 0.37 | 2.71 |
| mixed | 21,547 | 0.24 | 1.74 |

### Group windows: 100K Int64 rows (ms)

| Structure | Groups | Plugin | Native rank | Native rank_by | Band IEJoin | Equality join |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| sparse | 1 | 17.47 | 61.26 | 67.68 | 30.80 | skip |
| sparse | 10 | 11.57 | 17.47 | 51.93 | 27.54 | skip |
| sparse | 100 | 10.27 | 16.39 | 42.63 | 27.99 | skip |
| sparse | 1000 | 14.64 | 13.84 | 35.23 | 25.85 | skip |
| dense | 1 | 14.43 | 42.58 | 44.66 | skip | skip |
| dense | 10 | 11.62 | 16.14 | 44.76 | skip | skip |
| dense | 100 | 11.29 | 14.72 | 41.85 | skip | skip |
| dense | 1000 | 14.34 | 13.92 | 34.80 | 34.81 | skip |
| mixed | 1 | 11.18 | 20.53 | 22.57 | skip | skip |
| mixed | 10 | 10.64 | 12.63 | 23.66 | skip | skip |
| mixed | 100 | 7.83 | 13.73 | 26.08 | skip | skip |
| mixed | 1000 | 14.30 | 15.55 | 34.22 | 28.96 | skip |

### Temporal sample: 100K sparse rows (ms)

| Endpoint type | Groups | Plugin | Native rank | Native rank_by |
| --- | ---: | ---: | ---: | ---: |
| Date | 1 | 11.79 | 55.21 | 58.26 |
| Date | 100 | 11.67 | 14.89 | 43.69 |
| Datetime(time_unit='us', time_zone='UTC') | 1 | 13.39 | 57.43 | 61.60 |
| Datetime(time_unit='us', time_zone='UTC') | 100 | 10.24 | 14.17 | 44.71 |

### Small-group counterexamples: 10K rows / 1000 groups (ms)

| Structure | Plugin | Native rank | Native rank_by | Band IEJoin | Equality join |
| --- | ---: | ---: | ---: | ---: | ---: |
| sparse | 7.75 | 4.63 | 7.75 | 4.46 | 3.63 |
| dense | 7.22 | 4.88 | 8.47 | 5.42 | 3.94 |
| mixed | 7.33 | 5.27 | 8.10 | 5.05 | 3.85 |

Native formulations can beat the plugin with many tiny groups. This is
consistent with per-group plugin/extraction overhead becoming important,
although this benchmark does not isolate each source of that overhead.

### Run-rank follow-up: grouped and temporal samples (ms)

| Structure / dtype | Rows | Groups | Plugin | Native rank_runs |
| --- | ---: | ---: | ---: | ---: |
| sparse / Int64 | 10,000 | 1000 | 8.76 | 6.75 |
| sparse / Int64 | 100,000 | 1 | 14.95 | 54.41 |
| sparse / Int64 | 100,000 | 10 | 11.11 | 47.34 |
| sparse / Int64 | 100,000 | 100 | 9.48 | 35.50 |
| sparse / Int64 | 100,000 | 1000 | 14.65 | 31.98 |
| dense / Int64 | 10,000 | 1000 | 7.08 | 6.53 |
| mixed / Int64 | 10,000 | 1000 | 6.84 | 6.08 |
| sparse / Date | 100,000 | 1 | 11.45 | 50.63 |
| sparse / Date | 100,000 | 100 | 9.72 | 43.30 |
| sparse / Datetime(time_unit='us', time_zone='UTC') | 100,000 | 1 | 14.02 | 52.67 |
| sparse / Datetime(time_unit='us', time_zone='UTC') | 100,000 | 100 | 10.18 | 42.93 |

The raw reports contain every size, all three native rank formulations, grouped
join variants, all exact pair counts and every individual timing sample.

## Memory accounting

The Rust CSV records peak simultaneously live Vec capacity bytes, including
output, excluding caller-owned inputs, allocator metadata and stack frames.
This is deterministic allocation accounting, **not measured process RSS**.
There are four heap buffers per kernel: records/indices, compressed-coordinate
storage, ranks/output, and counter. Sorting is in place; dedup retains the
coordinate buffer's original capacity.

For i64 endpoints on this 64-bit machine and m unique ends:

| Candidate | Live Vec bytes | Maximum at 3M rows |
| --- | --- | --- |
| A | `40*n + 8*(m+1)` | 144,000,008 |
| B | `24*n + 8*(m+1)` | 96,000,008 |
| C | `40*n + 16*m` | 168,000,000 |

The shared Polars adapter borrows contiguous endpoint columns and collects only
columns spanning multiple chunks. Two collected buffers cost 16*n bytes for
Int64/Datetime or 8*n for Date; the original measured adapter always collected
both. It checks logical dtype equality before extracting Date/Datetime physical
integers. Group windows
can execute concurrently, so their peak process memory also depends on Polars
parallelism. We do not report native RSS or claim a measured allocation count
for Polars itself; join match counts explicitly show the pair-materialization
cost that the core and native rank formulations avoid.

## Reproduce

```sh
cargo bench -p intervals-core --bench containment --locked
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
uv run --locked --no-sync python benchmarks/containment_count.py
```

To reproduce the two recorded native runs separately, pass
`--methods plugin rank rank_by join bands equality` for the original run, then
`--methods plugin rank_runs` with a different `--output` for the follow-up.

Run benchmarks sequentially on an otherwise idle machine. `CONTAINMENT_SIZES`
accepts comma-separated kernel sizes and `CONTAINMENT_CSV` overrides its output
path. Python accepts `--sizes`, `--repeats`, `--methods` and `--output`. Defaults produce
`target/containment-kernels.csv` and `target/containment-polars.json`.
The latter includes environment metadata, all samples, join skips and query
plans. Windows Rust tests need the selected Python interpreter in `PYO3_PYTHON`
and its base directory on PATH so the Python DLL can be loaded.
On the recorded Windows host, Rust tests/Clippy/rustdoc used
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` to avoid oversized
debug-symbol artifacts. Kernel and wheel measurements use the ordinary optimized
release profile, without either setting affecting their optimization level.

## Validation

Named tests lock down nesting, disjoint/crossing/touching intervals, same-start
and same-end families, duplicates, boundary empties, empty outers, row order,
extreme signed/unsigned endpoints and error indices. The independent O(n²)
oracle uses only nested loops and the defining inequalities. An exhaustive
suite enumerates all 111,111 ordered collections of up to five intervals drawn
from ten intervals on [-1,2], cross-checking production and all three candidates.

`proptest` generates 0..=30 intervals by ordering pairs from [-8,8], without
filtering, at 512 cases per property. It tests oracle equality, length, bounds,
self exclusion, row-identity permutation equivariance, translation, positive
scaling, reflection, exact append deltas, duplicate insertion, own-count
monotonicity under expansion/shrinking, strict chains, identical families,
crossing antichains and the optional dual pair total. Production Fenwick and
reference Fenwick/segment counters have boundary/repeated-update tests and
random-operation comparisons against naïve arrays.

Installed-plugin tests cover eager/lazy select/with_columns, group windows and
aggregation, expressions, filtering after counting, mismatched chunk boundaries,
streaming, all integer widths, dates, datetime units/timezones, nulls, logical
dtype mismatch, empty input, invalid endpoints and no scalar broadcasting.

Before integration with the newer shared adapter, the local validation run
passed 87 Rust tests plus four Rust doctests, 171 Python
tests/doctests, and 49 CI/release-helper tests. It also passed Cargo formatting,
Clippy with warnings denied, Rust documentation with warnings denied, `uv lock
--check`, Ruff lint/format checks and strict MkDocs. A locally built release wheel
passed all 171 installed-package tests outside the checkout with both Polars
1.44.1 (the declared minimum) and 1.44.2. Wheel/sdist metadata, README, license,
typing marker, source inclusion, lockfile/toolchain preservation and strict
Twine checks passed. The full 15-wheel cross-platform release matrix remains a
CI responsibility; it was not run on this Windows host.

After integration with current master, the expanded workspace passed 166 Rust
tests and 12 Rust doctests, Clippy/rustdoc with warnings denied, the 49 CI/release
helper tests, three plotting tests, figure regeneration, and strict MkDocs.
A fresh release wheel passed all 1,416 Python tests/doctests outside the checkout
on both Polars 1.44.1 and 1.44.2. Native oracle checks, grouped/temporal cases,
join safety limits, and all three release kernels at 1K/10K were rechecked.
Wheel/sdist metadata, source inclusion and strict Twine validation also passed.

The plugin uses the repository's fallible output-field registration and shared
logical endpoint validation. Int128 remains unsupported. Existing temporal
support for the other operations is preserved; containment adds no endpoint
dependencies or separate extraction path.
