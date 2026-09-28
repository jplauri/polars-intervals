# Running and publishing benchmarks

The [overview](benchmarks.md) links to results for every operation and describes
the shared [hardware](benchmarks.md#hardware). Operation reports give commands,
timing exceptions, and links to run-specific metadata. The
[script inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md)
distinguishes benchmark runners from report generators.

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

**Timing and warmup.** Rust core calls and complete Polars collections measure
different scopes and must stay separate. Report whether validation, preprocessing,
planning, output materialization, and destruction are timed. Workload construction,
compilation, and correctness checks belong outside timing. Warm up before samples
and vary candidate order where the harness supports it. Warmup/sample counts,
thread settings, software versions, phase clocks, and allocator instrumentation
differ between saved runs; the report's scope line and metadata describe what
actually happened. These guidelines do not retroactively change historical runs.

**Correctness.** Validate outside timing against an independent oracle or a
documented cross-check. State which was used: agreement with another candidate,
a deterministic-mask check, and an independent optimum oracle are different
evidence. Check feasibility, dtype, and row order where applicable. Tied optimal
selections need not have identical masks. Detailed test inventories belong in
supporting notes or metadata.

**Runtime summaries.** Use medians within one run, workload, size, method, and
timing scope. Keep exact samples and min/max ranges accessible. Headline values
use about three significant figures. Ranges describe observed samples; overlap
is not a significance test, and a small median difference does not establish a
reliable winner. Do not pool separate runs or unrelated workloads. If reporting
relative runtime, define it as **candidate median / production median**:
production is 1.0× and values above 1.0× mean slower. This ratio of medians is
not a distribution of paired measurements. Identify the method whenever selecting
the fastest baseline independently per case.

**Limits and provenance.** Synthetic fixtures on one machine characterize those
workloads, not every application or platform. Keep losses, skips, bounded
baselines, and missing types visible. Preserve raw samples, environment, settings,
revisions/source hashes, and historical evidence in `benchmarks/results/`.
Core runners time production only; candidates that lost a saved comparison
were removed and remain available at the revision recorded in its metadata.
Discuss repeat runs separately, including meaningful variation or contradictions.

### Memory metrics

| Metric | What it means |
| --- | --- |
| Buffer capacity | Capacity of live algorithm buffers, often including output; excludes caller inputs, allocator bookkeeping, and RSS. Capacity accounting may exclude transient reallocations. |
| Requested live heap | Peak bytes requested from an instrumented allocator during a separate untimed call. Includes the objects named by the harness, excluding allocator overhead, stack, and RSS. |
| Process RSS | Resident memory for the whole process, including inputs and runtime. A query's high-water-mark increase is not an exact allocation count. Cold memory runs can differ from warmed timing runs. |

Name the applicable metric and exclusions beside results. Allocation counts,
buffer capacity, requested heap, and RSS cannot be substituted for one another.

## Generate plots

The same command generates compact tables and optional plots from saved data;
it never runs benchmark suites or builds Rust.

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The existing generator uses Polars aggregation and Matplotlib for optional SVGs.
Documentation CI regenerates assets before its strict build. Commit generated
`docs/assets/benchmarks/<id>.md` and `<id>.csv`; include `<id>.svg` only for a
configured chart. Markdown snippets are excluded as standalone site pages:

```text
;--8<-- "docs/assets/benchmarks/overlap-headline.md"
```

CSV downloads retain exact medians, min/max, sample counts, and table workload
dimensions. Missing measurements stay missing. Use `--output PATH` for a preview
or `--config PATH` for another configuration.

### Configure a table or chart

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

Plots are optional. Keep at most one per report, only when it clarifies scaling,
a crossover, a substantial workload difference, or a runtime/memory tradeoff.
A `[[charts]]` entry uses the same source/value/divisor/ylabel/methods, one
`filters` mapping, plus `id`, `title`, `scope`, and `caption`. Runtime plots
default to a logarithmic y axis and require positive values; memory plots use
`yscale = "linear"`. Lines show medians and bands show sample ranges. Close
comparisons usually belong in a table.

### Adding an operation

1. Reuse an appropriate runner and correctness checks; record samples, omissions,
   and run metadata without overwriting historical evidence.
2. Copy the [report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md)
   into `docs/`. Keep **Summary → Results → Coverage and limitations → Reproduce
   and data**, aiming for roughly 400–600 visible words without padding.
3. Add source mappings and a small representative table to `plots.toml`, including
   important production losses. Put end-to-end Polars results before separately
   labelled Rust comparisons. Give each set a scope line and metadata link.
4. Link shared methodology and hardware instead of repeating them. Put derivations
   and experiment history in existing supporting notes where possible. Include
   the operation command in a collapsed reproduction block.
5. Add the page to both the overview and `mkdocs.yml`; update the script inventory
   if needed. Preserve existing page URLs and repair affected links/anchors.
6. Run generation, reporting tests, and the strict build above. Check source
   agreement, generated assets, and representative wide/narrow rendered pages.
   Commit the report, configuration, and generated assets together.
