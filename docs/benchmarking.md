# Running and publishing benchmarks

The [overview](benchmarks.md) links to one report per operation. Use those reports
for workload-specific commands and the [script inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md)
to distinguish benchmark runners from report generators.

## Setup

Follow the [source build prerequisites](contributing.md#build-from-source).
Rust candidate benchmarks run with `cargo bench` and need neither Python nor
the Polars plugin. Python end-to-end benchmarks need a release build:

```sh
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
```

Then use `uv run --no-sync python ...` to preserve that build. Rebuild after
changing Rust code or dependencies. For an external wheel measurement, install
the release wheel into a separate environment and run its Python outside the
checkout. Individual reports describe the recorded setup.

Run benchmarks sequentially on an idle machine. Set `POLARS_MAX_THREADS` before
starting Python to control parallelism, and restore it afterwards. Save new runs
under new filenames rather than overwriting published measurements.

## Measurement rules

- **Correctness:** validate results outside timing against an independent oracle
  or documented scalable cross-check. Check feasibility, dtype, and output order
  where applicable. Tied optimal selections need not have identical masks.
- **Timing:** report what the timed region includes. Core calls and complete
  Polars collections are separate measurements. Exclude workload construction,
  compilation, and correctness checks. Warm up, vary candidate order, and retain
  every accepted sample. Summarize repeats within each workload before comparing
  workloads. Do not pool different sizes, distributions, or machines.
- **Memory:** name the metric and exclusions. Live buffer capacity, instrumented
  requested heap bytes, and process RSS are distinct. Keep instrumented allocation
  calls outside runtime measurement. An RSS high-water-mark increase is not an
  exact allocation count.
- **Evidence:** save raw samples, configuration, source revision/hashes, build
  settings, environment, and explicit omissions in `benchmarks/results/`.
  Repeat measurements before drawing conclusions. Preserve losing cases and
  describe uncertainty from small sample counts or one-machine measurements.

Existing reports document their historical measurement details, including
exceptions such as candidate phase-clock overhead. These rules do not change
what the saved runs measured.

## Generate plots

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The `plots` dependency group contains only the tools needed to process and plot
data. It does not build the plugin. The shared generator uses Polars to aggregate
samples and Matplotlib to render static SVGs. The documentation CI runs its checks
and regenerates figures before building the site. Checked-in figures also make
ordinary local documentation builds possible without plotting dependencies.

For each chart, `docs/assets/benchmarks/` contains:

| File | Purpose |
| --- | --- |
| `<id>.svg` | Figure with title, workload, axes, method labels, medians and sample ranges |
| `<id>.csv` | Exact plotted values: method, x, median, min, max and sample count |
| `<id>.md` | Figure, caption, source/download links and an expandable table from the same values |

Report pages include the generated Markdown with the same snippet syntax used
elsewhere in the documentation. For example:

```text
;--8<-- "docs/assets/benchmarks/lanes-runtime.md"
```

The Markdown fragments are excluded as standalone site pages. Commit all three
generated files together. Use `--output PATH` to preview generated assets in a
different directory, or `--config PATH` to use another chart configuration.

### Configure a chart

[`benchmarks/plots.toml`](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/plots.toml)
holds source column mappings and chart choices. Paths are relative to the
repository root. New CSV sources need one row per recorded sample, a `sample`
index, a size column, a method column, and numeric measurement columns. Existing
column names can be mapped directly. The original overlap-count JSON has a small
reader that flattens its `samples_ms`. It does not trust precomputed medians.

```toml
[sources.example]
path = "benchmarks/results/example-local.csv"
x = "n"
method = "algorithm"
dimensions = ["family", "order", "dtype"]

[[charts]]
id = "example-runtime"
source = "example"
title = "Example operation"
scope = "Rust core"
value = "ns"
divisor = 1000000
ylabel = "Runtime (ms)"
filters = { family = "disjoint", order = "shuffled", dtype = "i64" }
methods = { production = "Production", reference = "Reference" }
caption = "Describe this workload, the recorded environment, and any omissions."
```

List every independent workload dimension in the source: geometry, order, dtype,
weights, capacity, groups, seed, or any other factor that varies in that file.
Exclude derived output statistics such as optimum objective and concurrency
determined by the fixture. Every chart must fix all dimensions other than its
x coordinate and method. Use a single source file per run. Add a separate chart
for a repeat or another machine.

The generator rejects missing filters, empty selections, duplicate sample keys,
missing methods, negative measurements, warmup rows, and non-finite values.
Omitted method/size combinations remain gaps rather than zeroes or interpolated
measurements. It cannot discover dimensions omitted from the source declaration,
so review that list when adding a source. The x coordinate must be a positive
integer. Runtime charts also need positive values for their logarithmic y axis.

For a memory chart, change `value`, `divisor`, `ylabel`, and set
`yscale = "linear"`. For example, `buffer_bytes / 1048576` is MiB of live buffer
capacity. Describe the measurement scope in the caption. A new temporal CSV can
use `dtype` as its method/series column to compare dtypes within one workload.

### Adding an operation

1. Add the appropriate benchmark runner and correctness checks. Reuse an existing
   harness when operations share fixtures. A new Python runner is not mandatory.
2. Record samples and environment metadata, including cases skipped or not yet
   measured. Keep raw reports in `benchmarks/results/`.
3. Copy the [report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md)
   into `docs/`. Keep its seven section headings and label missing coverage.
4. Register the source and charts in `benchmarks/plots.toml`, generate the figures,
   and include them under the report's Results section. Inspect the figures and
   check their tables against the source samples.
5. Add the operation to the overview table and the Benchmarks navigation in
   `mkdocs.yml`. Add any new scripts to `benchmarks/README.md`.
6. Run the reporting checks and strict documentation build above. Commit the
   report, source data, chart configuration, and generated assets together.
