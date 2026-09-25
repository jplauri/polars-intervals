# Benchmarks

Compare equivalent outputs and state what each baseline measures.
Keep shared setup and measurement rules here, with one section per function.

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

## Measurement

- **Correctness:** check edge cases and compare every timed result with the
  reference, including dtype and row order. A mismatch aborts the run.
- **Runtime:** time complete `collect(engine="in-memory")` calls on prebuilt
  plans, including output materialization. Exclude fixture preparation, plan
  construction, compilation, and correctness checks. Warm up each method and
  vary method order; report medians from repeated collections.
- **Evidence:** retain samples, configuration, revision, and environment in
  JSON, including toolchain and build settings alongside published results.
  Repeat runs before drawing conclusions; hardware and parallelism matter.
  Distinguish measured memory use from algorithmic complexity.

## overlap_count

[Benchmark script](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py):
the plugin versus a Polars `join_where` self-join followed by counting. This
comparison shows the cost of enumerating overlap pairs; it does not establish
an advantage over an equivalent native counting expression.

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 100 1000 3000 --seed 42 --warmups 3 --repeats 10
```

Both workloads use shuffled, non-null `Int64` intervals. Sparse starts are four
units apart, with lengths 0–8; dense starts are 0–999 with lengths 0–1,000.
Both methods handle empty and touching intervals, duplicates, and self-exclusion,
returning `UInt64` counts in input order. Method order alternates each iteration.

Median query time in milliseconds; lower is better:

| Workload | Intervals | Overlapping pairs | Plugin | Polars join |
| --- | ---: | ---: | ---: | ---: |
| Sparse | 100 | 36 | 0.193 | 1.475 |
| Sparse | 1,000 | 393 | 0.190 | 2.128 |
| Sparse | 3,000 | 1,240 | 0.325 | 2.957 |
| Dense | 100 | 3,426 | 0.182 | 2.024 |
| Dense | 1,000 | 338,641 | 0.418 | 7.692 |
| Dense | 3,000 | 3,024,332 | 0.556 | 60.199 |

Measured 2026-09-25 on Windows 11, Ryzen 9 3900X, Python 3.14.0, Polars 1.44.2,
24 threads. Release build: Rust 1.98.1, uv 0.11.3, no custom Rust flags;
clean revision [`1f87983`](https://github.com/jplauri/polars-intervals/commit/1f87983b4c22e7e040b76e30333ca7ef23496648),
v0.1.0. Seed 42, three warmups, ten samples.
[Raw samples and environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json).

These are synthetic workloads on one machine. Peak memory was not measured.
The plugin takes `O(n log n)` time and `O(n)` additional space. The join can
materialize quadratically many pairs; large dense cases may exhaust memory.

## Adding a benchmark

Add `benchmarks/<function>.py` using the measurement rules above. Add one section
here with its baselines, run command, workload, compact results, and limitations.
Keep published raw reports in `benchmarks/results/`.
