# Benchmarks

See how `polars-intervals` performs as datasets grow and interval patterns change.
Choose an operation below to explore measured runtimes, compare approaches, and
understand the tradeoffs that matter for your workload.

| Operation | Purpose |
| --- | --- |
| [Overlap counting](overlap-count-benchmarks.md) | Count other intervals overlapping each row |
| [Coverage and load profiles](coverage-profile-benchmarks.md) | Measure coverage depth or resource demand |
| [Clustering, union, and gaps](interval-geometry-benchmarks.md) | Label connected intervals, merge their union, or find uncovered gaps |
| [Containment counting](containment-benchmarks.md) | Count other intervals contained by each row |
| [Nesting depth](nesting-depth-benchmarks.md) | Measure the longest strict containment chain above each row |
| [Lane assignment](assign-lanes-benchmarks.md) | Assign every interval to the fewest lanes without overlaps |
| [Lane balancing](balance-lanes-benchmarks.md) | Improve row-count balance while using the fewest lanes |
| [Weighted scheduling](weighted-scheduling-benchmarks.md) | Choose nonoverlapping intervals with maximum total weight |
| [Maximum-weight clique](clique-benchmarks.md) | Choose mutually overlapping intervals with maximum total weight |
| [Capacity scheduling](capacity-scheduling-benchmarks.md) | Maximize total weight under a simultaneous overlap limit |
| [Capacity profiles](capacity-profile-benchmarks.md) | Maximize total weight under capacity that changes over time |
| [Minimum covering](covering-benchmarks.md) | Cover a target with the fewest intervals |
| [Minimum-cost covering](cost-covering-benchmarks.md) | Cover a target at minimum total cost |
| [Minimum-cost dominating set](domination-benchmarks.md) | Choose minimum-cost representatives so every interval is selected or overlaps one |
| [Minimum stabbing points](stabbing-benchmarks.md) | Find the fewest points that hit every interval |
| [Maximum k-coverage](coverage-benchmarks.md) | Maximize covered length using at most `k` intervals |

## Hardware

Published runs use a fixed machine: **AMD Ryzen 9 3900X,
12 physical cores / 24 logical processors, 32 GiB RAM**,
running Windows 11 x86-64. The original hardware records remain in each run's
metadata for provenance.

Each result set links to its run settings. Rust core timings and complete Polars
timings are reported separately.

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
  guidance for concise operation reports.
