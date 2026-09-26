# Overlap counting benchmarks

[All benchmarks](benchmarks.md) · [Running and publishing](benchmarking.md)

## Summary

The production endpoint sweep counts overlaps in `O(n log n)` time and `O(n)` space
without materializing overlapping pairs. On the recorded synthetic matrix it usually
improves runtime at larger sizes. Native formulations can win on small groups and some
interval structures.

## Compared implementations

[Benchmark
script](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py):
the plugin versus two native `sort + search_sorted` layouts, sorted `join_asof` scans,
an endpoint sweep, and a `join_where` self-join. Native baselines assume valid, non-null
inputs. The plugin also validates them. All preserve empty-interval handling, half-open
boundaries, duplicates, self-exclusion, row order, and `UInt64` output.

| Method | Approach | Role |
| --- | --- | --- |
| `plugin` | Compiled endpoint sweep | Production |
| `native_expr` | Sorted endpoints with binary searches | Native expression baseline |
| `native_parallel` | Parallel layout of sorted-endpoint searches | Native expression baseline |
| `native_asof` | Sorted as-of joins | Native expression baseline |
| `native_sweep` | Endpoint events and prefix counts | Native expression baseline |
| `inequality_join` | Materialize matching interval pairs | Bounded join baseline |
| `plugin_reference` | Saved historical plugin binary | Optional revision comparison |

## Results

- **CPU:** AMD Ryzen 9 3900X (12 cores, 24 logical processors).
- **OS:** Windows 11, build 10.0.26200, x86-64.
- **Rust:** 1.98.1.
- **Python / Polars:** 3.14.0 / 1.44.2.
- **Build:** release plugin.
- **Threads:** 24 Polars threads for the main run. The one-thread run is reported separately.
- **Samples:** three warmups and nine timed samples per case.

--8<-- "docs/assets/benchmarks/overlap-runtime.md"

### Wider matrix and repeat runs

The table shows selected shuffled `Int64` medians in milliseconds. “Native” is the fastest of the four
native counting formulations, independently for each case:

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
- **Memory:** at 1M/3M rows, grouped peak RSS was 23–30% lower than the smallest
  native peak. Global peak RSS was 15–60% higher. Memory and timing winners can differ.
- **Algorithm:** two sorted endpoint streams and a linear sweep replace per-row
  binary searches. Total complexity remains `O(n log n)` time and `O(n)` space.
- **Join limit:** skip above 2M candidate rows to bound pair materialization.
  Grouped joins count equality-join candidates before overlap filtering.

## Workloads and correctness

This matrix covers sparse/dense overlaps, shuffled/start-sorted input, and
global/100-group counts. Sparse starts are four units apart within each group, with
lengths 0–8. Dense starts are 0–999 with lengths 0–1,000. The generator uses seed 42 and balanced groups.
Input sorting is outside timing. Optional datetime cases now pass logical Datetime
columns directly to the compiled plugin. Before timing, a small sanity check compares
Date and all Datetime units (including a timezone-aware case) with their physical
integer equivalents, both globally and within groups. The recorded historical runs used
explicit integer casts for datetime inputs. Saved reference binaries still receive
physical endpoints to allow comparisons with older integer-only versions. That conversion
is included in their timing.

The runner checks edge cases with an independent oracle and compares each timed output
to the reference, including dtype and row order. It times complete
`collect(engine="in-memory")` calls on prebuilt plans, including output materialization.
Fixture generation, plan construction and checks are outside timing. Methods are warmed
up and run in shuffled order. See the [shared measurement
guide](benchmarking.md#measurement-rules).

## Reproduce

| File | Role |
| --- | --- |
| [`overlap_count.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/overlap_count.py) | Run end-to-end plugin/native comparisons and optional RSS measurements |
| [`plot.py`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plot.py) | Generate figures and tables from saved samples |

Build the release plugin with the shared setup guide, then run:

```sh
uv run --no-sync python benchmarks/overlap_count.py --sizes 1000 100000 1000000 3000000 --dtypes int64 --warmups 3 --repeats 9 --memory
```

Use `--help` for workload options and `--output PATH` to retain a separate JSON report.
To compare revisions, save the old release binary before rebuilding and pass
`--compare-plugin PATH`.

## Limitations

These measurements use synthetic data on one Windows machine. The temporal results
include integer casts and do not measure direct Date/Datetime inputs.

RSS is the total peak process memory in a fresh process, with input loaded before
collection and its chunk layout restored. The query increase is a high-water-mark
change, not exact allocated bytes. Memory runs are cold while runtime measurements are
warmed.

## Raw data

[Main](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json),
[second-seed](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-repeat-windows.json),
and
[one-thread](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-one-thread-windows.json)
reports contain raw samples, binary hashes, and settings. The main run also compares the
binary-search plugin from `925033f`. Earlier join-only measurements remain in the
[original
report](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/v0.1.0-windows.json).
