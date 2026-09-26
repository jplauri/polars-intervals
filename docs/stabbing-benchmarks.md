# Minimum stabbing points benchmarks

[All benchmarks](benchmarks.md) · [Running and publishing](benchmarking.md)

## Summary

`minimum_stabbing_points` selects the minimum number of discrete points that hit every
half-open interval. Python and Rust Polars return one `List(endpoint_dtype)` per
collection. Grouped Python aggregation returns one list per group. The independent core
returns `Vec<T>` using a minimal `DiscreteEndpoint` trait.

Production uses packed end sorting with a direct scan for already end-sorted input.

## Compared implementations

All candidates use the same exact greedy rule: process increasing ends and choose
`predecessor(end)` when the last point does not hit the current interval. Validation
rejects empty/reversed intervals in original order before any sorting.

| Candidate | Representation and preprocessing |
| --- | --- |
| A | Copy packed `(start,end)` pairs, unstable comparison sort by end, linear scan |
| B | Keep endpoint arrays, unstable-sort `usize` indices by end, indirect scan |
| C | Scan borrowed arrays when ends are nondecreasing, otherwise use A |
| production | The public core function, timed without phase instrumentation |

Production retains **packed comparison sorting with the sorted-input fast path**.
Directly scanning sorted endpoints avoids allocating records. Indirect sorting saves
record storage but has less predictable memory access on shuffled inputs. Candidate code
stays private to tests and benchmarks. No selectable production backend or sorting
dependency is added. Optional radix sorting was not implemented. The fixed-width native
trait preserves compact physical widths and avoids allocating two widened i128 columns
in the existing adapter.

Sorting takes `O(n log n)` and the greedy scan `O(n)`. Additional space is `O(n + k)`
including `k` points. End-sorted inputs take `O(n)` time and `O(k)` output space.
Unsorted packed storage is `2 * n * sizeof(endpoint)` bytes, plus the growing output
vector. Sorting is in place. The Polars adapter borrows contiguous physical columns.
Multiple chunks require contiguous input copies.

## Results

Measured on September 26, 2026: Ryzen 9 3900X (12 cores / 24 logical processors),
Windows 11, Rust 1.98.1, Python 3.14.0 and Polars 1.44.2. The final measurements ran
sequentially without concurrent compilation or tests: **2,700 core samples** over 225
workloads and **270 release-wheel samples** over 90 workloads. Earlier development
timing passes were discarded.

--8<-- "docs/assets/benchmarks/stabbing-runtime.md"

### Full matrix and temporal measurements

Selected 3M-row Int64 core medians, milliseconds:

| Family / order | A packed | B indirect | C detect + packed | Production |
| --- | ---: | ---: | ---: | ---: |
| Disjoint / sorted | 31.38 | 24.59 | 17.84 | 17.77 |
| Disjoint / shuffled | 108.84 | 283.33 | 106.79 | 112.72 |
| Disjoint / reverse | 35.43 | **26.76** | 36.58 | 35.20 |
| Dense random / sorted | 22.58 | 15.34 | 7.67 | 7.44 |
| Dense random / shuffled | 99.31 | 230.37 | 98.53 | 98.27 |
| Dense random / reverse | 98.21 | 111.15 | 100.54 | 103.89 |
| Common intersection / reverse | 23.65 | **14.36** | 23.89 | 23.70 |
| Equal ends / shuffled | 23.93 | 17.15 | 9.59 | 9.22 |

C/A's geometric mean runtime ratio across all sorted sizes, families and physical widths
is **0.403** (about 2.48x faster). Excluding identical/equal-end families, which remain
sorted under permutation, the unsorted ratios are **0.988 shuffled** and **1.009
reverse**. The detection overhead is small beside the sorted-input benefit. At 3M
shuffled rows, packed records also clearly outperform indirect sorting. B wins some
reverse/monotone workloads and uses fewer temporary bytes for i64. There is no claim
that the selected implementation wins every case. Keeping the small C guard and one
packed fallback is the measured simplicity / performance tradeoff. Three samples do not
establish significance for small percentage differences, including differences between C
and production.

For 3M disjoint Int64 intervals, measured production peak requested heap is **33,554,432
bytes (32 MiB)** when sorted versus **81,554,432 bytes (77.78 MiB)** when shuffled. The
difference is the 48,000,000-byte record buffer. For the common intersection, sorted
input allocates only **32 bytes**, versus 48,000,032 bytes when shuffled. Both include
output vector capacity. The sorted path therefore retains its memory benefit even where
standard-library sorting is already linear.

Installed release-wheel medians for 3M rows, milliseconds:

| Family / order | Int64 | Date | Datetime(us) |
| --- | ---: | ---: | ---: |
| Disjoint / sorted | 26.47 | 15.01 | 27.64 |
| Disjoint / shuffled | 139.08 | 96.64 | 129.24 |
| Dense regular / sorted | 7.76 | 4.41 | 6.45 |
| Dense regular / shuffled | 114.73 | 78.70 | 113.79 |
| Identical / sorted | 6.44 | 4.33 | 7.78 |
| Identical / shuffled | 20.75 | 13.44 | 20.36 |

Core timings isolate the algorithm. These totals additionally include Polars and list
construction. Native data construction/shuffling and correctness checks are excluded.
Physical Date records are narrower than Int64/Datetime records.

## Workloads and correctness

Core controls `STABBING_BENCH_MAX` and `STABBING_BENCH_SAMPLES` default to 3M and three
samples. The seed is 20260926. Each workload has a verified, untimed allocation/warmup
pass, then three timing samples in shuffled method order. Inputs, independent oracles,
and result checks are outside the timed region. Core totals include validation,
record/index materialization, sorting, scanning and temporary-buffer destruction. Output
destruction is excluded. Candidate phase clocks distinguish validation, preprocessing
(including sortedness detection), sort, and scan. Production has no phase clocks. Its
phase CSV fields are intentionally empty. Clock overhead matters most at 1K.

The core matrix uses 1K, 10K, 100K, 1M and 3M intervals. All twelve families run with
end-sorted, reverse and shuffled orders: disjoint, common intersection, sparse random
overlap, dense random overlap, nested, touching, staircase/path overlap, identical,
equal ends, equal starts, random unit length, and very wide. Equal-end and identical
families remain end-sorted after reversal/shuffling. They exercise the fast path in all
orders. Random reversed data may have tied ends, which also affects standard-library
sort behavior.

Int64 covers the full matrix. Date's i32 physical kernel additionally covers
disjoint/dense/identical families in all sizes/orders. Datetime has the same i64 kernel
as Int64, so its huge core matrix is not repeated. The actual installed release wheel
measures Int64, Date and default microsecond Datetime on all five sizes,
disjoint/dense/identical families and sorted/shuffled orders. Native totals include lazy
collection, plugin dispatch, validation, physical adaptation and logical list
construction. These temporal workloads are regular interval families with explicit
packing certificates. The core random dense family differs.

### Independent correctness checks

Before recording times, each exact candidate is checked for coverage, strictly
increasing output and optimum cardinality. Large workloads use an independent
right-to-left maximum packing oracle: sort starts descending and accept an interval only
when its end does not cross the last accepted start. This oracle never computes stabbing
points. Each temporal benchmark checks every interval against its first candidate point
using an as-of join, and compares cardinality to a disjoint-packing certificate for that
family.

Small-instance tests enumerate subsets of **all integer coordinates** between the
minimum start and maximum end, not just predecessors of ends. A second oracle enumerates
subsets of intervals and checks every pair for disjointness. The benchmark cross-checks
both oracles on 256 small cases before running any timing.

The core has named regression tests for empty/single inputs, excluded touching
boundaries and chains, common intersections, duplicates, nesting and separated
components, disjoint intervals, the broad-first greedy counterexample, equal ends,
signed/unsigned limits and original-index errors. Seven proptest tests run 512 cases
each. The main generator directly produces 0–8 non-empty intervals in [-5,7]. Separate
generators inject empties, common points and disjointness. They cover all fourteen
requested properties: coverage, brute-force optimum, packing duality, sorted uniqueness,
determinism, permutation, translation, duplicates, added constraints, separated union,
common-point/disjoint cases, empty infeasibility, and predecessor-of-an-end as an
additional invariant.

Python tests exercise the installed native plugin, eager/lazy select, groups, window
list broadcasting, chunks, expressions, empty collections, all supported integer widths,
extreme values, Date, all three Datetime units and UTC/Helsinki metadata. Nanosecond
correctness is checked using physical values, avoiding Python datetime's microsecond
precision. Rust Polars tests also verify the list dtype and chunk adaptation. The
production Python wrapper contains no solver.

## Reproduce

| File | Role |
| --- | --- |
| [`minimum_stabbing_points.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/minimum_stabbing_points.rs) | Run Rust candidate comparisons |
| [`stabbing_summary.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/stabbing_summary.py) | Summarize the recorded core and temporal CSVs |
| [`stabbing_temporal.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/stabbing_temporal.py) | Time installed release-wheel integer and temporal queries |
| [`plot.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plot.py) | Generate figures and tables from saved samples |

Run optimized builds only, with no concurrent compilation or other benchmark:

```sh
cargo bench -p intervals-core --bench minimum_stabbing_points --locked > benchmarks/results/stabbing-local.csv
uv build --wheel --sdist --out-dir target/stabbing-dist --config-setting build-args=--locked
```

Install the wheel and Polars in an isolated environment outside the checkout, then run
its Python with `-I` and an external working directory:

```sh
python -I /path/to/checkout/benchmarks/stabbing_temporal.py > stabbing-temporal-local.csv
```

Keep the new temporal CSV under a separate local filename in `benchmarks/results/`. The
summary script reads the published filenames. Use the shared chart configuration to
select a separately named local run.

To summarize the existing published runs:

```sh
uv run --no-sync python benchmarks/stabbing_summary.py
```

## Limitations

Peak memory means live requested heap bytes and allocation/reallocation counts in a
separate invocation, excluding caller inputs, verification, stack, allocator metadata
and OS RSS. These are core measurements, not process-wide plugin memory.

There is no manufactured native-Polars speedup comparison. This is a sequential global
greedy selection problem without a clean equivalent in a small number of ordinary
dataframe expressions.

These are synthetic results from one Windows host. Three samples do not establish
significance for small percentage differences. Candidate phase clocks add overhead that
the production call does not have.

## Raw data

Raw samples and environment details are in [core
CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-windows.csv),
[release-wheel
CSV](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-temporal-windows.csv),
and [environment
metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-environment.json).
The summary script calculates medians directly from these files.

### Historical validation

These checks and test counts belong to the original measurement revision.

- `cargo fmt --check`, `cargo test --workspace --locked` (125 tests/doctests),
  `cargo clippy --workspace --all-targets --locked -- -D warnings` and strict
  `cargo doc --workspace --no-deps --locked`: passed.
- `uv lock --check`, `uv run --locked ruff check .` and
  `uv run --locked ruff format --check .`: passed.
- `uv run --locked pytest --doctest-modules python/polars_intervals tests`:
  1,295 passed. The same 1,295 passed against the release wheel from an external
  environment and working directory with Python `-I`.
- All 49 CI/release helper tests and
  `uv run --locked --isolated --only-group docs mkdocs build --strict`: passed.
- Local wheel/sdist metadata, license, typing marker, native library, Rust source,
  lockfile and pinned toolchain audits, and Twine strict checks: passed.

Local Rust tests needed the project Python interpreter in `PYO3_PYTHON` and its base
installation on `PATH` so Windows could locate the Python DLL. This is a
test-environment setup requirement, not a package dependency change.

These synthetic results describe one Windows x86-64 host, not a cross-platform speed
guarantee. Only the local CPython 3.14 Windows wheel is exercised here. The complete
15-wheel release matrix remains a CI check.
