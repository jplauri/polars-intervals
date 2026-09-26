# Benchmarks

## Setup

Follow the [source build prerequisites](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md#build-from-source),
then build the plugin in release mode from the repository root:

```sh
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
```

Run the command in the function's section below. Use `--no-sync` to preserve
the release build; rebuild after changing Rust code or dependencies. Run
benchmarks sequentially on an idle machine.

Set `POLARS_MAX_THREADS` before starting Python to control parallelism
(e.g. `$env:POLARS_MAX_THREADS = '1'` in PowerShell); restore it afterwards.
Use `--help` for workload options and `--output PATH` to save a separate report.
To compare revisions, save the old release plugin binary before rebuilding and
pass `--compare-plugin PATH` to time both binaries in the same run.

## Measurement

- **Correctness:** check edge cases against an independent oracle and compare
  every timed result with the reference, including dtype and row order.
- **Runtime:** time complete `collect(engine="in-memory")` calls on prebuilt
  plans, including output materialization. Exclude fixture preparation, plan
  construction, compilation, and correctness checks. Warm up each method and
  shuffle method order; report medians from repeated collections.
- **Memory:** measure total peak process RSS in fresh processes, with input
  loaded before collection and the original chunk layout restored. The query's
  peak increase is a high-water-mark change, not exact allocated bytes.
  Memory runs are cold; timing is warmed.
- **Evidence:** retain samples, configuration, revision, and environment in
  JSON, including toolchain and build settings alongside published results.
  Repeat runs before drawing conclusions; hardware and parallelism matter.

## overlap_count

[Benchmark script](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py):
the plugin versus two native `sort + search_sorted` layouts, sorted `join_asof`
scans, an endpoint sweep, and a `join_where` self-join. Native baselines assume
valid, non-null inputs; the plugin also validates them. All preserve empty-interval
handling, half-open boundaries, duplicates, self-exclusion, row order, and `UInt64` output.

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 1000 100000 1000000 3000000 --dtypes int64 --warmups 3 --repeats 9 --memory
```

This matrix covers sparse/dense overlaps, shuffled/start-sorted input, and
global/100-group counts. Sparse starts are four units apart within each group,
with lengths 0–8; dense starts are 0–999 with lengths 0–1,000. Seed 42; groups
are balanced. Input sorting is outside timing. Optional datetime cases include
the plugin's required integer cast in timing; its public API accepts integers.

Measured 2026-09-25 on Windows 11, Ryzen 9 3900X, Python 3.14.0, Polars 1.44.2,
24 threads, Rust 1.98.1 release build. Three warmups and nine samples per case.
Selected shuffled `Int64` medians in milliseconds; “native” is the fastest
of the four native counting formulations, independently for each case:

| Workload | Rows | Groups | Plugin | Native |
| --- | ---: | ---: | ---: | ---: |
| Sparse | 100,000 | 1 | 6.2 | 10.5 |
| Sparse | 1,000,000 | 1 | 71.6 | 106.4 |
| Sparse | 3,000,000 | 1 | 243.7 | 339.3 |
| Dense | 3,000,000 | 1 | 176.3 | 339.3 |
| Sparse | 3,000,000 | 100 | 213.7 | 384.0 |
| Dense | 3,000,000 | 100 | 166.9 | 376.4 |

- **Runtime:** the sweep plugin won all 24 cases at 100K–3M rows, by 1.20–2.77×.
  A second seed, 10/1,000 groups, and datetime casts also favored it (24 cases).
  With one thread it won all 16 cases, by 2.19–4.22×.
  Native can win with tiny groups, all-empty input, or nested intervals.
  These are synthetic results on one machine, not a general speed guarantee.
- **Memory:** at 1M/3M rows, grouped peak RSS was 23–30% lower than the smallest
  native peak; global peak RSS was 15–60% higher. Memory and timing winners can differ.
- **Algorithm:** two sorted endpoint streams and a linear sweep replace per-row
  binary searches. Total complexity remains `O(n log n)` time and `O(n)` space.
- **Join limit:** skip above 2M candidate rows to bound pair materialization.
  Grouped joins count equality-join candidates before overlap filtering.

[Main](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json),
[second-seed](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
and [one-thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json) reports contain raw samples,
binary hashes, and settings. The main run also compares the binary-search plugin
from `925033f`. Earlier join-only measurements remain in the
[original report](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json).

## Adding a benchmark

Add `benchmarks/<function>.py` using the measurement rules above. Add one section
here with its baselines, run command, workload, compact results, and limitations.
Keep published raw reports in `benchmarks/results/`.
