# Benchmarks

Saved measurements compare the production implementation with native Polars
expressions or Rust candidates. Start with an operation's results table. Each
report identifies its timing scope, workload, tradeoffs, and missing coverage.

| Operation | What was measured |
| --- | --- |
| [Overlap counting](overlap-count-benchmarks.md) | Complete Polars collections against native counting expressions |
| [Containment counting](containment-benchmarks.md) | Polars collections and separate Rust candidate comparisons |
| [Nesting depth](nesting-depth-benchmarks.md) | Polars collections and separate Rust candidate comparisons |
| [Lane assignment](assign-lanes-benchmarks.md) | Rust candidates for assigning the fewest nonoverlapping lanes |
| [Lane balancing](balance-lanes-benchmarks.md) | Row-count quality and runtime of construction and repair, in Polars and the Rust core |
| [Weighted scheduling](weighted-scheduling-benchmarks.md) | Rust candidates for maximum-weight nonoverlapping selection |
| [Maximum-weight clique](clique-benchmarks.md) | Exact maximum-weight intersection, unit defaults, Rust candidates and release Polars collections |
| [Capacity scheduling](capacity-scheduling-benchmarks.md) | Polars collections and Rust selection under an overlap limit |
| [Capacity profiles](capacity-profile-benchmarks.md) | Polars collections and Rust selection under changing capacity |
| [Minimum covering](covering-benchmarks.md) | Polars collections and Rust candidates for fewest-interval covering |
| [Minimum-cost covering](cost-covering-benchmarks.md) | Polars collections and Rust candidates for minimum-cost covering |
| [Minimum stabbing points](stabbing-benchmarks.md) | Polars collections and Rust candidates for interval hitting sets |
| [Maximum k-coverage](coverage-benchmarks.md) | Polars collections and Rust candidates for maximum covered measure |

## Hardware

Published runs use a fixed machine: **AMD Ryzen 9 3900X,
12 physical cores / 24 logical processors, 32 GiB RAM** (about 31.9 GiB usable),
running Windows 11 x86-64. The original hardware records remain in each run's
metadata for provenance.

Thread counts, sample counts, software versions, builds, and instrumentation vary
between runs. Read the scope line beside each result set and follow its metadata
link for the exact settings. Rust core timings and complete Polars collections
are separate measurements.

## Reading and reproducing results

Tables show selected median runtimes with explicit units, including important
production losses. Their downloadable data retain exact medians, sample ranges,
and counts. Detailed candidates and historical runs remain linked from each
report. See the [shared methodology](benchmarking.md#measurement-rules) for timing,
validation, memory definitions, and interpretation limits.

- [Running and publishing benchmarks](benchmarking.md): setup, regeneration, and
  a short checklist for adding an operation.
- [Script inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md):
  runners and report generators.
- [Report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md):
  the four-section structure used by every operation.
