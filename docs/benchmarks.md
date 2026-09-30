# Benchmarks

Find the runtime and Polars comparison for your operation below. These headlines
summarize full Polars queries on synthetic inputs. Open a report for the input
patterns, important exceptions and measured build.

## Interval queries and native Polars comparisons

Speedups compare the fastest equivalent native Polars query tested on the same
input. A missing comparison stays unmeasured.

| Operation | Purpose | Measured headline |
| --- | --- | --- |
| [Overlap counting](overlap-count-benchmarks.md) | Count overlaps for each row | 1.4–2.3× faster than Polars · 167–244 ms at 3M shuffled rows |
| [Coverage depth and resource demand](coverage-profile-benchmarks.md) | Measure active intervals or their total demand | 3.9–33× faster than Polars · 1.24–8.70 ms at 100k rows (one Polars thread) |
| [Clustering, union, and gaps](interval-geometry-benchmarks.md) | Label connected intervals, merge ranges or find gaps | Clustering: 7.5× faster than Polars · 4.11 ms at 100k shuffled rows (one Polars thread) |
| [Containment counting](containment-benchmarks.md) | Count containments for each row | 1.85–2.24× faster than Polars · 65.3–1,090 ms at 3M rows |
| [Nesting depth](nesting-depth-benchmarks.md) | Measure the longest containment chain above each row | About 200 ms at 3M shuffled rows · native comparison unmeasured |

## Optimizers without a built-in Polars solver

These reports show optimizer runtimes. Native Polars expression comparisons
have not been measured. Lane balancing is a heuristic for balance; the other
operations return an exact optimum for their stated objective.

| Operation | Purpose | Measured headline |
| --- | --- | --- |
| [Lane assignment](assign-lanes-benchmarks.md) | Assign the fewest lanes without overlaps | Exact · 59–176 ms at 1M shuffled rows |
| [Lane balancing](balance-lanes-benchmarks.md) | Improve row-count balance using the fewest lanes | Improved balance on 108/132 datasets · 3.76–24.1 ms at 100k rows (one Polars thread) |
| [Weighted scheduling](weighted-scheduling-benchmarks.md) | Select nonoverlapping intervals with maximum total weight | Exact · 96–260 ms at 1M shuffled rows |
| [Maximum-weight clique](clique-benchmarks.md) | Select mutually overlapping intervals with maximum total weight | Exact · 43.1 ms unweighted or 140 ms weighted at 1M shuffled rows |
| [Capacity scheduling](capacity-scheduling-benchmarks.md) | Maximize weight under an overlap limit | Exact · 28–119 ms at 1M rows in small independent overlap groups |
| [Scheduling with changing capacity](capacity-profile-benchmarks.md) | Maximize weight under an overlap limit that changes over time | Exact · 160 ms at 1M rows in small independent overlap groups |
| [Minimum covering](covering-benchmarks.md) | Cover a target with the fewest intervals | Exact · 51–69 ms at 1M shuffled rows |
| [Minimum-cost covering](cost-covering-benchmarks.md) | Cover a target at minimum total cost | Exact · 211–263 ms at 1M shuffled rows |
| [Minimum-cost dominating set](domination-benchmarks.md) | Select cheapest representatives so every interval is selected or overlaps one | Exact · 7.97–18.0 ms at 100k shuffled rows |
| [Minimum stabbing points](stabbing-benchmarks.md) | Find the fewest points that hit every interval | Exact · 26.5 ms sorted or 139 ms shuffled at 3M rows |
| [Maximum covered length](coverage-benchmarks.md) | Maximize covered length with a limited selection | Exact · 161–681 ms at 1M shuffled rows, selecting up to 8 or 64 intervals |

## Hardware

Published runs use a fixed machine: **AMD Ryzen 9 3900X,
12 physical cores / 24 logical processors, 32 GiB RAM**,
running Windows 11 x86-64. The original hardware records remain in each run's
metadata for provenance.

<span id="reading-and-reproducing-results"></span>

[Benchmark setup and reporting](benchmarking.md#reading-and-reproducing-results)
describes how to reproduce or contribute results.
