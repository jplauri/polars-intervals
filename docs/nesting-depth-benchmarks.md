# Nesting depth benchmarks and design

## Summary

`nesting_depth` returns the length of the longest strict containment chain above
each row. Outermost rows have depth zero. It is a global dynamic program over a
partial order, with exact duplicate geometries representing equivalent elements.
See [usage and endpoint semantics](usage.md#nesting-depth).

The production kernel uses packed records and a monotone vector frontier, with
a direct append when the deepest chain can extend. This choice removes end
compression while retaining exact row-aligned results. Overall worst-case time
is `O(n log n)`, with `O(n log(d+2))` DP work at maximum depth `d`, and total
additional space `O(n)`. Records and output take linear space; the frontier has
`d+1` entries with geometric allocation slack. Deep frontiers can use more memory
than a compressed tree, particularly when all ends are equal.

## Compared implementations

| Candidate | Approach | Role |
| --- | --- | --- |
| Packed Fenwick | Packed `(start,end,index)` records, compressed ends, prefix maximum over reversed end ranks | Exact candidate |
| Indirect Fenwick | Sort row indices and access the original endpoint arrays; same max recurrence | Exact locality/copying comparison |
| Iterative segment tree | Packed sort and compressed ends; iterative maximum query and point update | Exact alternative and cross-check |
| Vector frontier | Greatest achievable ending endpoint for each chain length | Production with direct append; binary-search-only comparison retained |

All candidates sort by start ascending, then end descending, then original index.
Every strict container precedes its child. Equal starts with different ends must
remain visible to one another; batching the entire equal-start run would be wrong.
Instead, process **only identical `(start,end)` geometries atomically**: determine
the common depth before inserting that geometry. Assign the result to every
original row in the batch. Equal ends at later starts are strict steps too.

The Fenwick recurrence uses reversed end ranks so `previous.end >= current.end`
is an inclusive prefix. The maximum stores chain **cardinality**, with zero
explicitly meaning no predecessor; a present row at depth zero stores one.
The returned depth is the maximum preceding cardinality. This avoids confusing
an absent predecessor with an outermost predecessor.

The frontier stores `tails[d]`, the **largest** end achievable by any processed
chain of cardinality `d+1`. These endpoints are nonincreasing: removing a last
member gives a shorter chain with an end at least as large. For a new geometry
ending at `e`, the number of tails `>= e` is the maximum predecessor cardinality.
Replace the following tail with `e`, or append if all tails qualify. Earlier
tails are already at least `e`; later tails cannot improve from a chain this
short. A single binary search and one update therefore preserve the invariant.
This is the monotone breakpoint idea in a contiguous vector; no ordered map,
pair graph, or endpoint arithmetic is needed. Duplicates perform one update.

The pilot exposed a weakness in binary-search-only frontier processing: when
many different starts share one end, the frontier grows by one at every step
and performs unnecessary searches. The append variant first checks the last
tail. If it is at least `e`, the deepest chain can extend immediately. This also
makes DP linear for a full chain after sorting. Both frontier variants remain
in the benchmark, each with and without an explicit sortedness scan.

An offline CDQ variant was not implemented: the frontier already removes end
compression with a simpler invariant, and three independent required kernels
remain available for comparison.

### Sorted-input decision

The production function always calls `sort_unstable_by`; it has no separate
sorted-input scan. Both Fenwick and frontier benchmarks tested an explicit
linear detection pass, with sorted, reverse, nearly sorted and shuffled input.
On this compiler the ordinary sort already handled fully sorted records just
as quickly as the scan. The added scan did not earn a separate production path.
This does not change the worst-case `O(n log n)` bound or promise a special
complexity contract for sorted callers. The raw samples retain both variants.

### Native Polars investigation

We inspected the installed Polars expression API and the official documentation.
[Rolling rank](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.rolling_rank.html)
can count ordered predecessors, but does not compute their recursively defined
chain depths. [Fold](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.fold.html)
combines columns horizontally. Neither supplies this row-to-row ordered state.
[Cumulative evaluation](https://docs.pola.rs/api/python/stable/reference/expressions/api/polars.Expr.cumulative_eval.html)
evaluates expanding input windows and documents potentially quadratic work;
it does not directly expose the earlier outputs of this dynamic program.

We found no clean exact formulation with comparable asymptotic complexity as a
small composition of ordinary dataframe operations. This is an investigation
result, not a proof that no native formulation is possible. We do not benchmark
a pair-materializing self join followed by longest-path iteration or claim a
speedup over such a baseline. `containment_count` and nesting depth solve
different problems: several incomparable containers contribute several counts
but at most one step above their common child.

## Results

The full core run records **4,640 accepted samples**: eight variants, five timed
samples for each of 100 main cases, four Date-physical cases and twelve grouped
cases. Candidate order rotates between samples. The pre-timing correctness and
allocation calls also warm each candidate; no warmup is included in the CSV.

- **CPU:** AMD Ryzen 9 3900X, 12 cores / 24 logical processors, 34.28 GB physical RAM.
- **OS:** Windows 11, build 26200, x86-64.
- **Rust:** 1.98.1, LLVM 22.1.8, `x86_64-pc-windows-msvc`.
- **Build:** Cargo release, default optimization level 3. Core calls are serial.
- **Scheduling:** runs are sequential, without concurrent repository builds or
  other benchmark runs. Measurements are local observations, not portable guarantees.

The table shows medians in **milliseconds**, at **3M Int64 rows, one group**.
Every cell has five raw samples; do not pool these different workloads.

| Structure | Packed Fenwick | Indirect Fenwick | Segment tree | Frontier + append |
| --- | ---: | ---: | ---: | ---: |
| Disjoint, shuffled | 1171.07 | 1466.64 | 1339.05 | 185.35 |
| One full chain, shuffled | 1221.15 | 1447.65 | 1454.86 | 199.48 |
| Dense random | 375.60 | 731.30 | 390.07 | 249.92 |
| Equal ends | 259.66 | 499.08 | 272.88 | 204.01 |
| All identical | 48.18 | **32.54** | 47.59 | 36.06 |
| Maximum depth 4 | 1114.22 | 1438.00 | 1394.72 | 196.22 |
| Maximum depth 64 | 1112.69 | 1438.17 | 1329.21 | 218.67 |
| Sorted dense | 126.04 | 113.48 | 145.16 | 41.47 |
| Reverse dense | 279.40 | 270.18 | 309.27 | 193.24 |
| Nearly sorted dense | 253.28 | 239.90 | 276.49 | 167.60 |
| Shuffled dense | 385.60 | 753.87 | 385.38 | 250.94 |

At this size, append-frontier beats packed Fenwick and segment tree in every
one of the twenty families, and indirect Fenwick in nineteen. **It is not the
winner on identical intervals:** the indirect candidate avoids copying endpoints
and takes 32.54 ms versus 36.06 ms. The selected kernel favors the wider workload
coverage rather than adding a workload-specific dispatch layer.

Compared with the binary-search-only frontier, appending reduces the full-chain
median from 335.13 to 199.48 ms and the equal-end median from 335.70 to 204.01 ms.
The largest observed 3M-row regression against that simpler frontier is 1.9%
on sparse input. Sample ranges remain relevant: append-frontier disjoint input
ranges from 182.12 to 210.25 ms, and the full chain from 198.74 to 220.89 ms.

For the explicit sortedness scan, 3M-row frontier medians change from 41.47 to
43.34 ms (sorted), 193.24 to 196.13 ms (reverse), 167.60 to 169.89 ms (nearly),
and 250.94 to 258.42 ms (shuffled). Sorted 1M-row input is effectively tied:
21.159 versus 21.179 ms. These observations do not justify retaining the scan.

Phase samples explain the larger differences. At 3M distinct-end rows, Fenwick
compression takes roughly 649–776 ms; the frontier omits it entirely. The
frontier then spends most time sorting: 129.71 ms sort and 49.80 ms DP/scatter
for a full chain, or 193.87 ms sort and 37.79 ms DP/scatter for dense random
input. Output scatter is fused into DP and recorded in that phase; output
allocation has its own column. Rank-buffer allocation belongs to compression
for tree candidates. These are independently computed phase medians, so they
need not sum exactly to the total median. Production repeat timings have no
internal phase clocks.

A second run records another **450 accepted samples**, including the public
production entry point without phase clocks, at 1M/3M rows on nine selected
families. At 3M, production medians are 214.70 ms (chain), 195.56 ms (equal
ends), 199.34 ms (depth 4), 252.11 ms (dense), 181.92 ms (crossing), 40.65 ms
(sorted), 189.30 ms (reverse), 166.75 ms (nearly sorted), and 253.78 ms
(shuffled). The corresponding packed Fenwick chain/depth-4/dense medians are
1301.84/1426.28/378.76 ms. The repeat supports the production choice while also
showing timing variation between runs. Explicit sorted detection is effectively
tied in the repeat's sorted case, 42.33 versus 42.44 ms, so the initial apparent
regression should be read as noise rather than a universal penalty.

--8<-- "docs/assets/benchmarks/nesting-shallow-runtime.md"

--8<-- "docs/assets/benchmarks/nesting-deep-runtime.md"

--8<-- "docs/assets/benchmarks/nesting-memory.md"

At 3M i64 rows, peak **requested live heap bytes** are 120.00 MB for packed
Fenwick, 72.00 MB for indirect Fenwick, and 120.00–144.00 MB for the segment
tree, each with four allocations. Frontier plus append uses 96.00 MB and three
allocations at depth zero, approximately 96.00 MB at depths 4/64, and 129.55 MB
with 23 allocations/reallocations on a full chain. Its vector grows geometrically;
the allocated capacity can exceed the number of active frontier entries. Thus
the chosen implementation trades higher deep-chain memory than Fenwick for
lower runtime, while using less than packed Fenwick on shallow families. The
chart uses MiB, whereas the numbers in this paragraph use decimal MB.

### Polars collection

The installed release-wheel run accepts **800 samples across 160 cases**, five
samples after one warmup. It uses Python 3.14.0, Polars 1.44.2 and
`POLARS_MAX_THREADS=4`, from outside the checkout in Python isolated mode. The
metadata records the installed native binary hash. Every full result agrees
with its analytic row-aligned depth array and has non-null UInt64 dtype.

At **3M shuffled rows, one group**, median collection times in milliseconds are:

| Family | Int64 | Date | Datetime(us) |
| --- | ---: | ---: | ---: |
| Maximum depth 7 | 202.69 | 170.43 | 200.33 |
| Full chain | 205.96 | 166.59 | 206.39 |

The Date adapter uses i32 physical endpoints; Int64 and Datetime share i64.
These observations measure the complete collection boundary, including lazy
optimization/planning and Series retrieval. The core and plugin generators
differ, so subtracting their times would not isolate adapter overhead.

At **100K shuffled Int64 rows**, actual window execution gives:

| Groups | Depth-7 families | Full chain per group | Duplicate-chain families |
| ---: | ---: | ---: | ---: |
| 1 | 5.12 | 4.79 | 5.32 |
| 10 | 6.28 | 5.43 | 5.66 |
| 100 | 5.93 | 5.20 | 6.00 |
| 1000 | 10.86 | 10.76 | 11.44 |

These medians are milliseconds at a fixed total row count, not fixed group
size. The duplicate family has depth 63 except at 1000 groups, where 100 rows
per group and two copies per geometry give maximum depth 49. Splitting the work
into more groups does not guarantee a faster complete Polars query; grouping,
scattering and per-call costs are included. Date and Datetime grouped samples
are retained in the raw data and summary.

The **3M-row Int64** ordering comparison is:

| Family | Sorted | Reverse | Nearly sorted | Shuffled |
| --- | ---: | ---: | ---: | ---: |
| Crossing antichain | 38.36 | 48.35 | 159.58 | 184.31 |
| Maximum depth 7 | 54.70 | 63.08 | 176.46 | 202.69 |
| Maximum depth 63 | 72.94 | 85.13 | 193.84 | 218.33 |
| Full chain | 42.64 | 53.50 | 173.48 | 205.96 |
| Duplicate-chain families | 50.14 | 191.61 | 178.06 | 208.83 |

The nearly sorted fixture swaps a late pair of distinct geometries. All times
use the production kernel without an explicit sortedness scan; the differences
reflect the ordinary sort's behavior on these particular ordered records.

--8<-- "docs/assets/benchmarks/nesting-temporal.md"

## Workloads and correctness

Core fixtures include disjoint intervals, crossing antichains, full chains,
balanced binary nesting, all-identical rows, eight copies per chain geometry,
equal starts, equal ends, repeated empty coordinates, mixed empties/nonempties,
sparse and dense random containment, broad outer intervals, heavy overlap with
little containment, and independent depth-4/depth-64 families. Sorted, reverse,
nearly sorted and shuffled variants use the same dense geometry multiset.
The fixed generator seed is 42. Every one of the twenty families runs at 1K,
10K, 100K, 1M and 3M rows. Four 100K-row families also run with i32 Date physical
endpoints and with 10/100/1000 serial independent groups; the one-group case
is already present in the main matrix. Serial core grouping measures call-size
effects, while the plugin runner measures actual grouped Polars execution.

The core candidates are checked against an independent quadratic oracle before
timing small fixtures; full-size depth arrays must agree before samples are
accepted. All endpoint comparisons use the exact strict containment predicate.
The Python runner separately checks small fixtures against a quadratic oracle,
then checks every timed full output against analytic depth values carried through
the input permutation. Fixture construction, dtype conversion, expression
construction and output validation are excluded. Lazy optimization/physical
planning inside `collect`, collection and result Series retrieval are timed.

The plugin matrix covers Int64 at 1K, 10K, 100K, 1M and 3M rows, with crossing
antichains, depth 7 and 63 families, full chains and replicated depth-63 chains.
Each runs sorted, reverse, nearly sorted (a late inversion), and shuffled.
Date and Datetime(us) cover shallow and deep shuffled families at the same sizes.
At 100K total rows, 1/10/100/1000 groups cover antichains, shallow/deep chains and
duplicates across all three dtypes. Temporal casts are untimed; Date uses the
i32 kernel and Datetime uses i64, avoiding a redundant matrix of timestamp units.
Unit tests separately cover ms/us/ns and supported named timezones.

The production tests include named regressions for every tie and empty-endpoint
case, first-invalid-row validation, extremes of signed and unsigned integers,
original-order reconstruction, and a 100K-row chain. A memoized recursive
quadratic oracle directly follows the strict containment predicate without
sharing sorting, compression or tree logic. Exhaustive testing covers all
111,111 ordered collections of up to five rows over the ten valid geometries
whose endpoints lie in `[-1,2]`. For up to four rows, an independent subset
enumerator recognizes chains by pairwise strict comparability.

Proptest generates 0–25 intervals over `[-8,8]`, intentionally producing ties,
empties and crossings. The properties cover exact oracle agreement, length and
bounds, strict-container lower bounds, zero-depth characterization, permutation,
translation, positive scaling and reflection, duplicate insertion, own-depth
monotonicity under expansion/shrinking, constructed deep chains/antichains/
identical families/duplicate chains, strict-container count bounds, and global
maximum chain cardinality checked by subsets on tiny cases. A separate property
cross-checks `containment_count` against strict children plus identical rows.
Benchmark candidates have their own oracle properties, rank-compression checks,
duplicate batching regressions, and random Fenwick/segment-tree update/query
traces, including absence versus a present depth zero.
The production frontier has direct append/replacement/duplicate tests and a
property that recomputes the entire achievable-end frontier from an independent
quadratic DP after **every** processed geometry batch, not only at final output.

## Validation

The workspace Rust test run passes **327 tests and doctests**. A separate fixed
seed run (`PROPTEST_RNG_SEED=20260927`, `PROPTEST_CASES=4096`) passes all 63
focused core/candidate tests, including 23 property functions and 94,208 generated
cases, in addition to the exhaustive collections described above.

The Python suite and API doctests pass **1,975 tests** against the installed
native plugin. The complete suite also passes outside the checkout with Python
isolated mode for each of three installations: the release wheel on Polars
1.44.2, the same wheel on the minimum Polars 1.44.1, and a separately built
source distribution. The release smoke explicitly checks native nesting depth.
The 49 release/CI-script tests, strict Twine metadata checks, source inclusion,
license and package metadata checks pass. See the
[package verification record](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-package-checks-windows.json)
for artifact hashes, commands and external import locations. These local checks
cover CPython 3.14 on Windows x86-64; the full cross-platform wheel matrix remains
a CI check.

Formatting, lock consistency, Ruff lint/format, all-target Clippy with warnings
denied, Rust documentation with warnings denied, strict MkDocs, all benchmark
figure generation and the three plot-reporting tests pass. The required commands
are `cargo fmt --check`, `cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`uv lock --check`, `uv run --locked ruff check .`,
`uv run --locked ruff format --check .`,
`uv run --locked pytest --doctest-modules python/polars_intervals tests`, and
`uv run --locked --isolated --only-group docs mkdocs build --strict`.
Rust documentation additionally uses `RUSTDOCFLAGS="-D warnings"` with
`cargo doc --workspace --no-deps --locked`.

Windows Rust tests required `PYO3_PYTHON` pointing to the virtual environment,
the base Python directory on `PATH` for its runtime DLL, and
`CARGO_BUILD_JOBS=1` to stay within this machine's paging capacity. The initial
unconstrained build exhausted virtual memory; the serialized run passed without
source changes.

## Reproduce

Use the [benchmark setup](benchmarking.md), run measurements sequentially, and
install a **release** plugin before end-to-end measurements. The core benchmark
has no Polars dependency. The recorded full run uses:

```powershell
$env:NESTING_CSV = Join-Path (Get-Location) "target/nesting-kernels.csv"
$env:NESTING_SAMPLES = "5"
$env:NESTING_METHODS = "A_packed_fenwick,A_sorted_fenwick,B_indirect_fenwick,C_packed_segment,D_frontier,D_sorted_frontier,D_append_frontier,D_sorted_append_frontier"
cargo bench -p intervals-core --bench nesting_depth --locked
```

`NESTING_SIZES` and `NESTING_SCENARIOS` accept comma-separated selections for a
smaller experiment; defaults run all five sizes and twenty structures.
`NESTING_METHODS` can select comma-separated candidates, including `production`
for the uninstrumented public function. The current default also includes
`production`; the explicit selection above reproduces the eight-variant matrix.
For the recorded repeat, use sizes
`1000000,3000000`, methods
`A_packed_fenwick,D_frontier,D_append_frontier,D_sorted_append_frontier,production`,
and scenarios `chain,equal_ends,depth_4,dense,crossing,sorted,reverse,nearly,shuffled`.

```powershell
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
$env:POLARS_MAX_THREADS = "4"
uv run --no-sync python benchmarks/nesting_depth.py --output target/nesting-polars.csv
```

For a quick plugin run, add `--sizes 1000 --group-size 1000 --repeats 2`.
The recorded plugin measurements use an external release-wheel environment.

| File | Purpose |
| --- | --- |
| [Core runner](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/nesting_depth.rs) | Candidate timings, phases, allocation measurements and verification |
| [Candidate kernels](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/support/nesting_depth.rs) | Reproducible exact alternatives and fixtures |
| [Plugin runner](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/nesting_depth.py) | Integer/temporal and grouped collection samples, analytic and quadratic checks |

## Limitations

These are measurements from one Windows desktop, not a universal hardware
ranking. Core and Polars collection timings have different scope. Peak requested
heap bytes exclude caller-owned endpoints, allocator metadata and process RSS;
allocation instrumentation runs separately from runtime measurements. Plugin
memory is not measured. Each core case has one allocation measurement, repeated
in its five timing rows; allocation sample counts are not independent repeats.
No native Polars speedup is asserted.

## Raw data

- [Full core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv)
  and [environment/source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.json).
- [Core median/min/max/phase summary](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-summary-windows.csv).
- [Production repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-windows.csv),
  [environment/source hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-windows.json),
  and [summary](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-production-summary-windows.csv).
- [100K pilot samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-pilot-windows.csv)
  and [environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-pilot-windows.json),
  before adding the append variant. Pilot summary numbers are not used in the result tables.
- [Plugin samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv)
  and [installed-wheel environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.json),
  with [per-case median/min/max summary](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-summary-windows.csv).
