# Overlap counting benchmarks

This standalone harness compares the compiled Rust plugin accessed through
`pi.overlap_count` with a Polars inequality self-join. It uses the existing locked
dependencies and Python's standard library; `pytest` does not run benchmarks.

## Reproduce

Follow the
[source installation prerequisites](../README.md#source-and-development-installation),
then run from the repository root:

```sh
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
uv run --no-sync python benchmarks/overlap_count.py
```

The first command explicitly rebuilds the plugin in release mode. `--no-sync`
then uses that prepared environment without rebuilding with different backend
settings; repeat the sync command after changing Rust code or dependencies.
Run on an idle machine, with the same toolchain, Python, lockfile, thread count,
and build flags when comparing revisions. Do not use a debug plugin. Record
`rustc --version`, `uv --version`, and any custom Rust build flags alongside
published results.

The command-line defaults are defined in [overlap_count.py](overlap_count.py).
To choose dataset sizes and measurement settings explicitly:

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 100 500 2000 --seed 42 --warmups 3 --repeats 10
```

Use `--scenarios sparse` or `--scenarios dense` to run one scenario. The console
shows median milliseconds and the output path. The JSON report records every
sample, median/min/max, actual overlap density, matching join rows, arguments,
Git revision/dirty state, Python/package versions, platform, CPU description,
and Polars thread count. Use `--output PATH` to retain separate runs. Results are
ignored by Git under `target/`; keep the JSON with any reported measurements.

Polars uses its default thread pool. To control it, set `POLARS_MAX_THREADS`
**before** starting Python (for example, `POLARS_MAX_THREADS=4 uv run ...` in a
POSIX shell, or `$env:POLARS_MAX_THREADS = '4'` in PowerShell). The Rust counting
algorithm itself is currently single-threaded; both queries use the same Polars
thread pool setting.

## Data and equivalent results

Both scenarios use shuffled, non-null `Int64` intervals generated with a local
`random.Random(seed)`, reset for each scenario/size:

- **Sparse:** starts are `4 * i`, lengths are uniformly sampled integers from
  0 through 8. Only immediate neighbors can overlap, so matching pairs grow
  linearly with the number of rows. Lengths include empty and touching cases.
- **Dense:** starts are uniformly sampled integers from 0 through 999, and
  lengths from 0 through 1,000. The endpoint domain stays fixed as row count
  grows, creating many overlaps and quadratically growing matching pairs.

The input is generated once per case and shared by both methods. Lengths are
added to starts, so every interval is valid. These are synthetic workloads,
not a model of all real interval distributions.

Both queries produce just one `UInt64` count per original input row, in input
order. The baseline uses [`LazyFrame.join_where`](https://docs.pola.rs/user-guide/transformations/joins/)
with `left.start < right.end` and `right.start < left.end`, enabling Polars'
inequality join optimizer. It filters empty intervals on **both** sides first,
groups matches by the original left row ID, and subtracts exactly one self-match
per non-empty row. Duplicate intervals remain distinct rows. A left join back
to all original row IDs restores zeros for empty intervals and preserves order.
Strict inequalities make touching intervals non-overlapping.

Before timing, fixed examples check empty inputs, empty intervals, self-exclusion,
touching intervals, duplicates, nesting, partial overlaps, dtype, and row order.
Both methods are also compared on every generated dataset and every measured
result; any mismatch aborts the run. There is no Python overlap algorithm.

## Timing and interpretation

Logical lazy plans are built once outside timing. Each sample times a complete
`collect(engine="in-memory")` using `perf_counter_ns`, including query optimization,
plugin execution or join/grouping/zero-fill/order restoration, and output
materialization. Repeated collections execute the query again; results are not
reused. Data generation, plan construction, compilation, imports, correctness
checks, result disposal, and reporting are excluded. Garbage collection remains
at Python's default setting. Method order alternates each iteration; both get
the same warmup and measurement counts. The initial correctness collections also
warm both methods, so this measures repeated execution, not cold-start latency.

The plugin counts in `O(n log n)` time without enumerating matching pairs.
The join baseline enumerates directed matches (including self) before aggregation;
the JSON records that cardinality. Dense cases can require quadratic memory and
time just to process those matches. Defaults are deliberately modest; increasing
dense sizes substantially may exhaust memory. Peak memory is **not** measured.

Compare raw samples and repeat whole runs before drawing conclusions. Tiny cases
can be dominated by execution overhead, and timings vary with hardware, threads,
versions, and background load. This harness supports comparisons only for these
workloads and this baseline, not claims about every Polars approach or universal
speedups. It sets no performance pass/fail thresholds.
