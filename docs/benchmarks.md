# Benchmarks

Find measured runtimes for your operation below. The tables summarize full
Polars queries on synthetic inputs. Open a report for input patterns,
comparisons, and limitations.

## Interval queries and native Polars comparisons

Speedups compare the fastest equivalent native Polars query tested on the same
input. Several reports show native Polars matching or beating the package on
many tiny groups, such as ten or fewer rows per group.

| Operation | Purpose | Measured headline |
| --- | --- | --- |
| [Overlap counting](overlap-count-benchmarks.md) | Count overlaps for each row | At least 1.3× faster than Polars · under 250 ms for 3M rows |
| [Coverage depth and resource demand](coverage-profile-benchmarks.md) | Measure active intervals or their total demand | At least 1.2× faster than Polars · under 96 ms for 1M rows |
| [Per-query coverage statistics](coverage-stats-benchmarks.md) | Count source records and measure covered length for every query | At least 8.1× faster than Polars · under 480 ms for 1M queries against 1M sources |
| [Clustering, union, and gaps](interval-geometry-benchmarks.md) | Label connected intervals, merge ranges or find gaps | At least 1.1× faster than Polars · under 16 ms for 100k rows |
| [Subtraction and intersection](set-geometry-benchmarks.md) | Remove or intersect covered ranges across two collections | Under 41 ms for 1M total input rows · slower than Polars on empty-only grouped inputs |
| [Containment counting](containment-benchmarks.md) | Count containments for each row | At least 1.2× faster than Polars · under 1.1 s for 3M rows |
| [Nesting depth](nesting-depth-benchmarks.md) | Measure the longest containment chain above each row | Under 210 ms for 3M rows (four Polars threads) · native comparison unmeasured |

## Optimizers without a built-in Polars solver

These reports show optimizer runtimes. Lane balancing is a heuristic for
balance; the other operations return an exact optimum for their stated
objective.

| Operation | Purpose | Measured headline |
| --- | --- | --- |
| [Lane assignment](assign-lanes-benchmarks.md) | Assign the fewest lanes without overlaps | Exact · under 180 ms for 1M rows |
| [Lane balancing](balance-lanes-benchmarks.md) | Improve row-count balance using the fewest lanes | Improved balance on 108/132 datasets · under 25 ms for 100k rows (one Polars thread) |
| [Weighted scheduling](weighted-scheduling-benchmarks.md) | Select nonoverlapping intervals with maximum total weight | Exact · under 270 ms for 1M rows |
| [Maximum-weight clique](clique-benchmarks.md) | Select mutually overlapping intervals with maximum total weight | Exact · under 150 ms for 1M rows (one Polars thread) |
| [Capacity scheduling](capacity-scheduling-benchmarks.md) | Maximize weight under an overlap limit | Exact · under 120 ms for 1M rows in small independent overlap groups |
| [Scheduling with changing capacity](capacity-profile-benchmarks.md) | Maximize weight under an overlap limit that changes over time | Exact · under 170 ms for 1M rows in small independent overlap groups |
| [Minimum covering](covering-benchmarks.md) | Cover a target with the fewest intervals | Exact · under 70 ms for 1M rows |
| [Minimum-cost covering](cost-covering-benchmarks.md) | Cover a target at minimum total cost | Exact · under 270 ms for 1M rows |
| [Minimum-cost dominating set](domination-benchmarks.md) | Select cheapest representatives so every interval is selected or overlaps one | Exact · under 18 ms for 100k rows (one Polars thread) |
| [Minimum stabbing points](stabbing-benchmarks.md) | Find the fewest points that hit every interval | Exact · under 140 ms for 3M rows |
| [Maximum covered length](coverage-benchmarks.md) | Maximize covered length with a limited selection | Exact · under 690 ms for 1M rows, selecting up to 64 intervals |

## Hardware

Published runs use a fixed machine: **AMD Ryzen 9 3900X,
12 physical cores / 24 logical processors, 32 GiB RAM**,
running Windows 11 x86-64. The original hardware records remain in each run's
metadata for provenance.

<span id="reading-and-reproducing-results"></span>

[Benchmark setup and reporting](benchmarking.md#reading-and-reproducing-results)
describes how to reproduce or contribute results.
