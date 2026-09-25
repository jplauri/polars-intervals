# Overlap counting benchmarks

Compare `overlap_count` with a Polars inequality self-join that produces the
same counts.

## Results

Median query time in milliseconds; lower is better.

| Workload | Intervals | Overlapping pairs | `overlap_count` | Polars join |
| --- | ---: | ---: | ---: | ---: |
| Sparse | 100 | 36 | 0.193 | 1.475 |
| Sparse | 1,000 | 393 | 0.190 | 2.128 |
| Sparse | 3,000 | 1,240 | 0.325 | 2.957 |
| Dense | 100 | 3,426 | 0.182 | 2.024 |
| Dense | 1,000 | 338,641 | 0.418 | 7.692 |
| Dense | 3,000 | 3,024,332 | 0.556 | 60.199 |

Measured September 25, 2026 on Windows 11 with an AMD Ryzen 9 3900X,
Python 3.14.0, Polars 1.44.2, and 24 Polars threads. The plugin was built in
release mode using Rust 1.98.1 and uv 0.11.3, with no custom Rust flags.
Revision: [`1f87983`](https://github.com/jplauri/polars-intervals/commit/1f87983b4c22e7e040b76e30333ca7ef23496648)
(v0.1.0, clean checkout). Each case used seed 42, three warmups, and ten timed runs.
[Raw samples and environment](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json).

These are synthetic workloads on one machine. The gap grows with dense overlaps
because the join enumerates matching pairs. Timings vary with hardware, versions,
thread count, and background load. Peak memory was not measured.

## Reproduce

Follow the [source build prerequisites](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md#build-from-source),
then run from the repository root:

```sh
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
uv run --no-sync python benchmarks/overlap_count.py --sizes 100 1000 3000 --seed 42 --warmups 3 --repeats 10
```

The first command rebuilds the plugin in release mode. `--no-sync` keeps that
build for the benchmark; repeat the sync command after changing Rust code or
dependencies. Run on an idle machine. Use the same toolchain, lockfile, thread
count, and build flags when comparing revisions.

Use `--scenarios sparse` or `--scenarios dense` to run one workload, and
`--output PATH` to keep separate reports. Defaults are defined in
[overlap_count.py](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py).
The JSON report includes every sample, overlap density, join row count, Git
revision, versions, and machine details. Keep it with any published measurements,
along with `rustc --version`, `uv --version`, and custom build flags.

Polars uses its default thread pool. To change it, set `POLARS_MAX_THREADS`
before starting Python (`POLARS_MAX_THREADS=4 uv run ...` in a POSIX shell,
or `$env:POLARS_MAX_THREADS = '4'` in PowerShell). The counting algorithm is
single-threaded; both queries use the same Polars thread pool setting.

## Workloads and correctness

Both workloads use shuffled, non-null `Int64` intervals. A local
`random.Random(seed)` is reset for each workload and size.

- **Sparse:** starts are `4 * i`, with lengths sampled uniformly from 0 through 8.
  Only immediate neighbors can overlap, so matching pairs grow linearly.
- **Dense:** starts are sampled uniformly from 0 through 999, with lengths from
  0 through 1,000. The fixed endpoint range creates quadratically growing pairs.

Both methods receive the same input and return one `UInt64` count per original
row, in input order. The baseline uses
[`LazyFrame.join_where`](https://docs.pola.rs/user-guide/transformations/joins/)
with strict overlap inequalities. It removes empty intervals from both sides,
groups by the original row ID, subtracts one self-match, and restores zero counts
and row order with a left join. Duplicate intervals remain distinct rows.

Fixed examples check empty input, empty and touching intervals, self-exclusion,
duplicates, nesting, partial overlaps, dtype, and row order. Every generated
case and timed result is checked for equality; a mismatch aborts the run.

## What is timed

Each sample times a complete `collect(engine="in-memory")`, including query
optimization, execution, and output materialization. Plans are built once;
each collection executes again. Data generation, plan construction, compilation,
imports, correctness checks, result disposal, and reporting are outside timing.
Garbage collection stays at its default setting.

Method order alternates each iteration, with equal warmup and measurement
counts. Correctness checks also warm both methods, so these measurements
represent repeated execution rather than cold starts. Repeat whole runs before
drawing conclusions; small cases can be dominated by execution overhead.

## Algorithm and memory

For `n` intervals, the plugin sorts non-empty starts and ends independently,
then uses binary searches to count overlaps. This takes `O(n log n)` time and
`O(n)` additional space, preserving input order.

The join produces directed matches, including self-matches, before aggregation.
The report records that intermediate row count. Dense cases can require
quadratic memory and time; substantially increasing their size may exhaust
memory. The memory comparison here describes the algorithms, not measured usage.
