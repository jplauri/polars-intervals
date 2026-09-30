# Running and publishing benchmarks

This guide covers collecting measurements, generating tables and publishing
results. The [overview](benchmarks.md) links to reports and shared
[hardware](benchmarks.md#hardware).

## Reading and reproducing results

Tables show median runtimes with explicit units, including cases where the
package is slower. Downloads retain exact medians, sample ranges and counts.
Each report links its raw measurements and historical records.

- [Report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md):
  the single source for report content, layout and comparison conventions.
- [Script inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md):
  benchmark runners and table generators.
- [Measurement rules](#measurement-rules): timing, validation and provenance.

## Setup

Follow the [source build prerequisites](contributing.md#build-from-source).
Rust core benchmarks use `cargo bench` without Python or the plugin.
Python end-to-end benchmarks need a release build:

```sh
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
```

Use `uv run --no-sync python ...` to preserve that build; rebuild after code or
dependency changes. To measure an external wheel, install it in a separate
environment and run its Python outside the checkout. Run benchmarks sequentially
on an idle machine. Set `POLARS_MAX_THREADS` before starting Python, restoring it
afterwards. Save new runs under new filenames.

## Measurement rules

**Timing and warmup.** Record timing boundaries in metadata, including validation,
preprocessing, planning, output materialization and destruction. Keep Rust core
calls and full Polars queries separate. Workload construction, compilation and
correctness checks belong outside timing. Warm up before samples and vary
candidate order where supported. Record warmups, samples, software versions,
phase clocks and allocator instrumentation. For default-thread measurements,
leave `POLARS_MAX_THREADS` unset and record the resulting pool size. These rules
do not retroactively change historical runs.

**Correctness.** Validate outside timing against an independent oracle or a
documented cross-check. Record the verification method. Check feasibility,
objective, dtype and row order where applicable. Tied optimal selections need
not have identical masks. Preserve detailed checks in notes or metadata.

**Runtime summaries.** Calculate medians within one run, workload, size, method
and timing scope. Preserve exact samples and min/max ranges. Sample-range
overlap is not a significance test, and a small median difference does not
establish a reliable winner. Do not pool separate runs or unrelated workloads.
Follow the report template for displayed comparisons and ratio conventions.

**Limits and provenance.** Synthetic fixtures on one machine characterize those
workloads, not every application or platform. Record skips, bounded baselines
and missing types. Preserve raw samples, environment, settings, source/build
hashes and historical evidence in `benchmarks/results/`. Retain repeat runs
separately. Never relabel old timings as measurements of changed code.

<details markdown="1">
<summary>Recovering historical runs</summary>

Most core runners time production only. The coverage-profile runner retains
private event, heap and weighted-index comparisons because they are needed to
reproduce its production decision. Removed older candidates and their runners remain
available in the [pre-cleanup snapshot at `d742de3`](https://github.com/jplauri/polars-intervals/tree/d742de3e57fdc523d1673e81106ca6d5109127bb).
Some run metadata records a base revision with uncommitted feature changes, so
that revision may not contain the benchmark code. Use the snapshot to recover
the suite and consult each run's source hashes and cleanup notes for differences
from the measured sources.

</details>

### Memory metrics

| Metric | What it means |
| --- | --- |
| Buffer capacity | Capacity of live algorithm buffers, often including output; excludes caller inputs, allocator bookkeeping, and RSS. Capacity accounting may exclude transient reallocations. |
| Requested live heap | Peak bytes requested from an instrumented allocator during a separate untimed call. Includes the objects named by the harness, excluding allocator overhead, stack, and RSS. |
| Process RSS | Resident memory for the whole process, including inputs and runtime. A query's high-water-mark increase is not an exact allocation count. Cold memory runs can differ from warmed timing runs. |

Record the metric and exclusions in the measurement data. Allocation counts,
buffer capacity, requested heap and RSS cannot be substituted for one another.

## Generate tables { #generate-plots }

The same command generates compact tables from saved data;
it never runs benchmark suites or builds Rust.

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The generator uses Polars aggregation only. Documentation CI regenerates assets
before its strict build. Commit generated `docs/assets/benchmarks/<id>.md` and
`<id>.csv`. Markdown snippets are excluded as standalone site pages:

```text
;--8<-- "docs/assets/benchmarks/overlap-headline.md"
```

CSV downloads retain exact medians, min/max, sample counts, and table workload
dimensions. Missing measurements stay missing. Use `--output PATH` for a preview
or `--config PATH` for another configuration.

Compact report tables can include only their table rows with `:3:-3` after the
snippet filename. Include the generated download links with `:-2:` inside
**Benchmark details**. This keeps exact data accessible without duplicating
the timing label or showing source links in the main report.

### Configure a table

[`benchmarks/plots.toml`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plots.toml)
holds source mappings and display choices. Each source is **one run**. CSV files
need a sample index, size, method, and numeric measurement columns. The JSON
reader flattens saved `samples_ms`, rather than trusting stored medians.

```toml
[sources.example]
path = "benchmarks/results/example-local.csv"
x = "n"
method = "algorithm"
dimensions = ["family", "order", "dtype"]

[[tables]]
id = "example-runtime"
source = "example"
value = "ns"
divisor = 1000000
ylabel = "Runtime (ms)"
methods = { production = "Heap (production)", reference = "Endpoint sweep" }
cases = [
  { label = "Shuffled disjoint Int64", sizes = [1000, 100000], filters = { family = "disjoint", order = "shuffled", dtype = "i64" } },
]
```

List every independent workload dimension: geometry, order, dtype, weights,
capacity, groups, seed, and any other varying input. Exclude derived output
statistics. Each case fixes all dimensions other than size and method; use labels
that make those choices understandable. Optional case `methods` selects a subset
of the table's methods when a baseline is unavailable for that whole case.
Unmeasured cells show a dash, never zero. Use separate tables for separate runs.

The generator rejects unfixed dimensions, empty selections, duplicate sample
keys, absent requested methods, negative or non-finite measurements, and warmup
rows. It cannot discover a dimension omitted from the source declaration, so
review that list. A requested size with no samples is an error; missing individual
method/size combinations remain missing.

For a native comparison, set `native_comparison = { package = "production",
baselines = ["native"] }` on the table. Include every equivalent native method
measured in that run. The table chooses the lowest native median separately
for each workload and size. Exact downloads retain all method summaries.
Case `inconclusive_sizes = [1000]` displays "about the same" where the evidence
does not support a dependable winner. Make that decision from the samples and
repeat runs, not an arbitrary percentage threshold.

### Adding an operation

1. Reuse an appropriate runner and correctness checks; record samples, omissions,
   and run metadata without overwriting historical evidence.
2. Copy the [report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md)
   into `docs/` and follow it.
3. Register the report's generated tables in `plots.toml`.
4. Add the page and its measured headline to the overview and `mkdocs.yml`.
   Update the script inventory if needed. Preserve page URLs and repair
   affected links/anchors.
5. Run generation, reporting tests, and the strict build above. Check source
   agreement, generated assets, and representative wide/narrow rendered pages.
   Commit the report, configuration, and generated assets together.
