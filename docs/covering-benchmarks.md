# Minimum covering benchmarks

[All benchmarks](benchmarks.md) · [Running and publishing](benchmarking.md)

## Summary

`minimum_cover` selects the fewest intervals that continuously cover one half-open
target. Production uses packed candidates and a start-sorted greedy sweep: `O(n log n)`
time and `O(n)` additional space. It performs well on varied shuffled input. Indirect
sorting can win on ordered or repeated geometry.

For the weighted operation from the same harness, see [minimum-cost
covering](cost-covering-benchmarks.md).

## Compared implementations

| Candidate | Implementation | Role |
| --- | --- | --- |
| MC-A | Packed candidates, start sort, linear greedy sweep | Production and instrumented benchmark |
| MC-B | Original-index sort, indirect greedy sweep | Tests/benchmarks |
| MC-C | Start sort, max-heap reference | Tests/benchmarks |
| MC-A-detect | MC-A with an explicit linear sortedness check | Tests/benchmarks |

Packed sorting avoids scattered endpoint access on varied shuffled data. Indirect
sorting saves candidate storage. The heap is a distinct cross-check. No explicit
sorted-input fast path is retained: the standard sort already handles ordered input
well, and the extra scan has inconsistent benefits.

### Semantics and exactness

All intervals and the single target are half-open. Touching intervals chain. Empty
targets return all false after validation. Empty intervals never help. Outside rows are
discarded after validation, and useful rows are clipped to the target. Reversed
targets/intervals and null endpoints are errors. A non-empty target without a continuous
cover raises `target interval cannot be covered by the supplied intervals`.

Sort packed `(effective_start, effective_end, row)` records and linearly sweep. Among
every interval beginning at or before the current frontier, choose the furthest end.
Every scanned alternative ends at or before that new frontier and can be discarded. An
exchange argument establishes optimality: replacing an optimal cover's first advancing
interval with this furthest-reaching interval never increases the required remaining
intervals. Repeat on the remaining target suffix. Equal effective ends prefer the
original row index.

The result is a Boolean mask in original row order. Rust Polars accepts two scalar
targets with exact matching endpoint dtypes. The independent core accepts slices and
scalar endpoints. Contiguous physical endpoint columns are borrowed. Multiple chunks
require copying. The core has no production dependencies.

## Results

Measured on Windows 11 x86-64, AMD Ryzen 9 3900X (12 cores / 24 threads), Rust 1.98.1,
Python 3.14 and Polars 1.44.2, with the optimized Cargo bench profile. The shared
covering run has 7,425 timed samples across 396 workloads and 2,475 candidate/workload
combinations, with three repeats each. These counts cover both operations, not just the
operation on this page.

--8<-- "docs/assets/benchmarks/cover-runtime.md"

### Wider workload comparison

Median per-workload runtime ratios (candidate / packed baseline). Values below one favor the candidate.
Each geometry/order/cost/size combination has equal weight:

| Candidate | Sorted | Nearly sorted | Shuffled |
| --- | ---: | ---: | ---: |
| MC-A-detect / MC-A | 0.944 | 0.984 | 1.000 |
| MC-B / MC-A | 0.880 | 1.015 | 1.339 |
| MC-C / MC-A | 1.662 | 1.458 | 1.207 |

Sorted/shuffled include the physical-width runs. Nearly sorted does not. Selected 1M-row
shuffled medians, milliseconds:

| Workload | Packed | Indirect | Heap |
| --- | ---: | ---: | ---: |
| Touching chain | 50.07 | 84.18 | 54.84 |
| Dense overlaps | 47.48 | 65.37 | 64.24 |
| Equal starts | 13.85 | 8.99 | 30.05 |
| Mostly irrelevant | 0.91 | 0.93 | 0.88 |
| Date-width chain | 38.79 | 72.23 | 51.50 |
| Date-width dense | 47.05 | 79.82 | 69.78 |

For the 1M-row i64 chain, packed/indirect peak requested allocation is 26.17/9.39 MB.
Packed preprocessing/sweep medians are 45.95/3.09 ms, with 20 allocation/reallocation
events. Mostly irrelevant rows need about 1 MB, primarily for the output mask.

Sortedness detection changes the 1M sorted chain from 13.86 to 14.11 ms and the sorted
Datetime-width dense case from 14.26 to 15.28 ms. Benefits elsewhere were too
inconsistent to justify an extra production pass.

### End-to-end temporal measurements

The release wheel installed outside the checkout produced 1,536 samples across 512
dtype/workload combinations for both covering operations. These selected 1M-row
shuffled-chain medians include plugin, validation, adapter and output overhead. The
native fixtures and costs differ from the core harness. Subtracting their times does not
estimate adapter overhead.

| Endpoint dtype | minimum_cover (ms) |
| --- | ---: |
| Int32 | 39.99 |
| Int64 | 69.03 |
| UInt64 | 52.47 |
| Date | 39.83 |
| Datetime(ms) | 52.91 |
| Datetime(us) | 52.61 |
| Datetime(ns, UTC) | 52.58 |
| Datetime(ns, Europe/Helsinki) | 51.64 |

## Workloads and correctness

--8<-- "benchmarks/covering-methodology.md"

## Reproduce

| File | Role |
| --- | --- |
| [`covering.rs`](https://github.com/jplauri/polars-intervals/blob/master/crates/intervals-core/benches/covering.rs) | Run both covering candidate suites |
| [`covering_summary.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering_summary.py) | Summarize a supplied candidate CSV |
| [`covering_temporal.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/covering_temporal.py) | Time both operations with integer/temporal release-plugin inputs |
| [`plot.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plot.py) | Generate separate figures for each operation |

Build the release plugin using the shared setup guide before running the temporal suite.
Save new runs separately from the published measurements:

```sh
cargo bench -p intervals-core --bench covering --locked > benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_summary.py benchmarks/results/covering-local.csv
uv run --no-sync python benchmarks/covering_temporal.py > benchmarks/results/covering-temporal-local.csv
```

`COVER_BENCH_MAX` and `COVER_BENCH_SAMPLES` limit candidate size and repeats. Defaults
are one million rows and three samples. The harness measures both operations. The chart
configuration selects the relevant method IDs.

## Limitations

Raw CSV columns retain every sample's total, preprocessing (validation, clipping,
sorting), and sweep time. Greedy selection writes the mask during its sweep, so the
reconstruction field is zero. Total time also includes temporary-buffer destruction and
phase-clock overhead. Early failures skip later phases.

Peak live requested allocation bytes and allocation/reallocation event counts are
measured in a **separate untimed invocation** using the system allocator. These include
algorithm buffers and output, but exclude caller-owned inputs, oracle work, allocator
metadata, stacks and process RSS. Measurements are serial.

These synthetic workloads were measured on one Windows x86-64 machine. Short timings
are sensitive to scheduler and clock noise. Candidate comparisons cover Rust
implementations, with separate end-to-end plugin timings and no native Polars baseline.

## Raw data

- [Shared candidate samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv)
- [Completion log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.log)
- [Environment and hashes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-environment.json)
- [Installed release-wheel samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv)

The original source hashes remain attached to the measurements. Cleanup verification
records the consolidated harness/adapter sources separately. The measured solver and
candidate implementations were unchanged.

<details markdown="1" id="historical-validation">
<summary>Validation at the recorded revision</summary>

The completed local validation includes 99 Rust tests/doctests, 1,229 Python
tests/doctests (also all passing against the installed external release wheel), and 49
CI/release helper tests. Formatting, Clippy across all targets with denied warnings,
Rustdoc with denied warnings, the uv lock check, Ruff lint/format and strict MkDocs
builds pass. A release wheel and source archive were built and audited for metadata,
license, type marker, native extension and new source files. The full 15-wheel
cross-platform CI matrix is not a local test and was not run.

</details>
