# Weighted scheduling benchmarks

[All benchmarks](benchmarks.md) · [Running and publishing](benchmarking.md)

## Summary

Select a globally maximum-weight subset of mutually non-overlapping intervals.
Production uses **B: two sorted orders and a linear predecessor sweep**, followed by
exact dynamic programming and reconstruction. Time is `O(n log n)` and additional space
is `O(n)`.

## Compared implementations

All candidates validate the original rows, omit nonpositive weights, and select positive
empty intervals separately. Empty intervals conflict with nothing. Their objective is
added to the ordinary schedule with checked `i128` arithmetic.

| Candidate | Preparation | Optimization and reconstruction |
| --- | --- | --- |
| A | Sort by `(end, start, original index)` and store sorted ends | Binary-search each compatible prefix, then run linear DP and backtracking |
| B | Use the same finish order. Sort finish positions independently by `(start, end, original index)` and sweep sorted ends to find prefixes | Linear DP and backtracking |
| C | Sort `(coordinate, event kind, original row)` events with ends before starts at equal coordinates | Snapshot the best objective and chain head at each start. Compare the completed candidate at its end, then reconstruct via saved heads |

For A/B, `OPT[j + 1] = max(OPT[j], weight[j] + OPT[p[j]])`, with `p` a prefix length.
Non-empty intervals guarantee `p[j] <= j`. A strict improvement selects the row. Ties
skip it. Original row indices complete every sort key, so identical input is
deterministic. These internal tie rules are not a stable public promise. C processes all
ends at a coordinate before starts, so touching intervals can chain. Separating empties
prevents start/end ordering from losing their weights.

Only B ships in the core. A/B/C references live in
`crates/intervals-core/benches/support/weighted.rs`, shared by tests and benchmarks. C
is retained there for reproduction and differential testing. Its larger event buffers
and generally slower measurements do not justify production complexity.

### Polars boundary

The public plugin is non-elementwise and returns Boolean masks, with independent
instances under windows/grouped aggregation. Eight signed/unsigned integer weight dtypes
up to 64 bits are accepted. The adapter widens them exactly to `i128`, while endpoints
reuse the existing integer/Date/Datetime validation and physical buffers. It performs no
floating-point conversion or implicit casts.

The existing `polars-core` dependency has its Decimal type-import feature enabled so the
FFI schema can reach our explicit unsupported-weight error for Decimal and Int128. This
adds no new crates or Decimal IO/SQL features, and does not accept those weight types.
Tests require the descriptive integer-weight error instead of accepting a generic
plugin-panic message.

## Results

- **CPU:** AMD Ryzen 9 3900X (12 cores, 24 logical processors).
- **OS:** Windows 11 Home, build 10.0.26200, x86-64.
- **Memory:** 31.9 GiB usable RAM.
- **Rust:** 1.98.1, LLVM 22.1.8, target `x86_64-pc-windows-msvc`.
- **Python / Polars:** 3.14.0 / 1.44.2, used for reporting and integration checks only.
- **Build:** default optimized Cargo `bench` profile, default target flags and allocator.
- **Threads:** one, without CPU affinity settings.
- **Samples:** two warmups and five timed samples per case.

Core timings use `i64` endpoints and weights, with checked `i128` objectives.

--8<-- "docs/assets/benchmarks/weighted-runtime.md"

### Full matrix and repeat run

Across all 528 workloads, B's geometric-mean runtime relative to A is **0.794**. C's is
**1.528**. Per-workload medians give 390 wins to B, 114 to A, and 24 to C. Each workload
has equal weight in this aggregate, independent of size/runtime. An independent repeat
with identical inputs gives **0.798** for B and **1.555** for C relative to A, with
389/119/20 wins for B/A/C. Both complete runs validate every output (15,840 recorded
candidate samples in total).

Representative first-run medians, **1M rows, positive weights, milliseconds**:

| Structure | Order | A | B (production) | C |
| --- | --- | ---: | ---: | ---: |
| Disjoint | Finish-sorted | 41.87 | 21.03 | 27.28 |
| Disjoint | Shuffled | 147.78 | 81.94 | 204.01 |
| Moderate overlap | Finish-sorted | 46.34 | 20.88 | 109.07 |
| Moderate overlap | Shuffled | 173.23 | 100.84 | 169.74 |
| Nested | Shuffled | 132.77 | 78.13 | 138.42 |
| Many empties | Shuffled | 55.77 | 39.62 | 46.94 |
| Random lengths | Finish-sorted | 82.81 | 120.22 | 118.13 |
| Random lengths | Shuffled | 198.58 | 240.06 | 153.08 |

**Why B:** the linear sweep removes substantial predecessor-search cost on the
structured families, including shuffled inputs. Its second sort and temporary
buffer are a small implementation addition, with the same peak vector storage
as A. It wins the measured workload mix while keeping a conventional DP and
backtracking implementation. No algorithm-selection heuristic is needed.

**Where it loses:** random lengths scramble the second sort: across that family's
48 cases, B takes 1.609 times A's runtime geometrically. Several expensive-interval
cases also favor A, and C wins some large shuffled cases. B is a measured tradeoff,
not universally superior. A workload mix dominated by independently ordered
starts and finishes would justify reconsidering the production choice.

For shuffled disjoint positive rows at 1M, coarse phase medians (milliseconds) show
where the time goes:

| Candidate | Preparation | Optimization | Reconstruction |
| --- | ---: | ---: | ---: |
| A | 50.08 | 94.26 | 3.00 |
| B | 65.63 | 11.66 | 3.05 |
| C | 88.78 | 68.88 | 39.61 |

### Timing and memory interpretation

The coarse phase timers use only three start/elapsed pairs per call. Preparation
includes validation, empty handling and sorting. B also includes its predecessor sweep
there. A performs binary searches inside the optimization phase. The DP or event sweep
includes its buffers and checked objective accumulation. Reconstruction measures only
following links/scanning DP entries and setting output bits. Total time also includes
buffer destruction, small timer/reporting costs, and function overhead. Medians of
phases need not add up to median total. Small measurements are especially sensitive to
timer and operating-system noise.

Memory is **peak simultaneously live vector capacity**, including the byte-per-row Rust
Boolean output, not process RSS. Allocation counts are derived from the nonempty
buffers, not an instrumented allocator. There are no vector growth reallocations or
allocating stable sorts. Inputs, stack frames, allocator metadata, and the independent
oracle are excluded. With `n` input rows and `m` positive non-empty rows on this 64-bit
target:

| Candidate | Peak buffer bytes | Nonempty-buffer allocations when `m > 0` |
| --- | ---: | ---: |
| A | `9n + 32m + 16` | 5 |
| B | `9n + 32m + 16` | 6 |
| C | `9n + 72m` | 5 |

B frees the extra `8m` start-order vector before allocating `16(m+1)` DP bytes, so it
adds an allocation without increasing peak live buffer capacity over A. At 1M positive
non-empty rows A/B use 41,000,016 bytes (39.10 MiB). C uses 81,000,000 bytes (77.25
MiB). The Polars adapter additionally materializes an `i128` weight vector (16 bytes per
row), borrows contiguous endpoint buffers, copies endpoints only for multiple chunks,
and builds Polars Boolean output. These adapter costs are outside this core-only
benchmark.

## Workloads and correctness

The full matrix crosses 1K, 10K, 100K and 1M rows with both finish-sorted and shuffled
input, eleven structures, and six weight distributions: **528 workloads**. The
deterministic xorshift generator starts at seed 42. Two warmups and five recorded runs
per candidate shuffle candidate order, giving 7,920 samples per full run. Workload
construction and all correctness checks are outside timing.

| Structure | Intervals before sorting, for `i = 0..n-1` |
| --- | --- |
| Mostly disjoint | `[3i, 3i + 2)` |
| Low overlap | `[i, i + 8)` |
| Moderate overlap | `[i, i + 128)` |
| Dense overlap | `[i, i + n)` |
| Deep nesting | `[i, 2n - i)` |
| Long staircase | `[i, 2i + 2)` |
| Duplicates | `[4 floor(i/16), 4 floor(i/16) + 8)` |
| Equal starts/ends | `[16 floor(i/32), 16 floor(i/32) + 16 + (i mod 4))` |
| Touching | `[i, i + 1)` |
| Many empties | Start `floor(i/4)` with length zero for two thirds of rows and 8 otherwise |
| Random lengths | `[i, i + 1 + random(0..n/8))` with less-correlated start/finish orders |

The six weight distributions are:

- Uniformly positive, from 1 to 100.
- Mixed signs, from -100 to 100.
- About 80% zeros, with remaining values from 1 to 100.
- Small ties, from 1 to 3.
- Powers of two, from 1 through `2^62`.
- An expensive interval competing with many weight-10 rows.

In the last case row zero is replaced by an
interval spanning the whole ordinary input with weight `5n`. It wins in some dense cases
but loses to the combined disjoint schedule.

Before any timing is accepted, every candidate and production are checked against an
independent **start-sorted suffix DP**: at each row, skip it or take its weight plus the
optimum starting at the first start at least its end. It shares no candidate
preparation, finish ordering, predecessor links or reconstruction. Every timed mask is
checked for original-row length, selected positive empties, positive weights,
non-overlap, and that independently computed objective. Tied masks need not match.

Test-only exhaustive subset enumeration independently validates all candidates,
production, and the suffix oracle on small inputs. Proptest generates 0–12 rows, starts
in -4–4, lengths in 0–6 and signed weights -10–20 without filtering. Properties cover
feasibility, exact objective, shape, determinism, translation, row permutation,
negative-row additions, positive-empty additions, all-negative weights and positive
scaling. Additional deterministic tests cover greedy counterexamples, predecessor
chains, duplicates, integer boundaries, overflow, length validation and reversed
original row indices.

## Reproduce

| File | Role |
| --- | --- |
| [`max_weight_non_overlapping.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/max_weight_non_overlapping.rs) | Run Rust candidate comparisons |
| [`weighted_summary.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/weighted_summary.py) | Summarize a supplied candidate CSV |
| [`plot.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plot.py) | Generate figures and tables from saved samples |

```sh
cargo test -p intervals-core --locked
cargo bench -p intervals-core --bench max_weight_non_overlapping --locked > benchmarks/results/weighted-local.csv
uv run --no-sync python benchmarks/weighted_summary.py benchmarks/results/weighted-local.csv
```

Set `WEIGHTED_BENCH_MAX_N=1000` for a smoke run. Run timings on an idle machine,
separately from compilation or other benchmarks. The benchmark asserts release mode. The
core benchmark needs neither Polars nor Python. The summary script uses the project's
existing Polars dependency.

## Limitations

These measurements cover the Rust core on one Windows machine with one endpoint
width. They exclude Python/Polars overhead and a native Polars baseline. Ordering,
overlap, hardware, and compiler can affect the relative timings. The random-length
workloads test cases where start and finish order are less correlated.

## Raw data

[first
run](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv),
[repeat](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-repeat-windows.csv),
[environment and source
hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-environment.json).

<details markdown="1" id="historical-validation">
<summary>Validation at the recorded revision</summary>

Completed locally on the environment above (Python 3.14.0):

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 65 tests and 6 doctests passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo doc --workspace --no-deps --locked`, `RUSTDOCFLAGS=-D warnings` | Passed |
| `uv lock --check` | Passed |
| `uv run --locked ruff check .` | Passed |
| `uv run --locked ruff format --check .` | Passed |
| `uv run --locked pytest --doctest-modules python/polars_intervals tests` | 485 passed |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed |
| CI/release helper tests and version/toolchain policy | 49 tests passed and policy passed |
| Release candidate benchmark | Two complete, verified 528-workload runs |
| Release wheel installed outside checkout, `python -I ...release_checks.py installed` | 485 passed with both Polars 1.44.2 and minimum Polars 1.44.1 |
| Wheel/source archive metadata and `twine check --strict` | Passed |

The wheel is a local CPython 3.14 Windows x86-64 release build. The installed artifact
checks confirm imports come from a separate temporary environment, exercise
integer/Date/Datetime smoke cases, then run the full tests and doctests. The source
archive was built and inspected for the implementation, tests, benchmark reference,
lockfile and compiler policy. Other OS/Python wheel targets remain covered by the
existing CI release matrix. They were not run locally.

</details>
