# Benchmarks

Explore benchmarks for each available operation below.

| Operation | Problem |
| --- | --- |
| [Overlap counting](overlap-count-benchmarks.md) | Count the other intervals overlapping each interval. |
| [Lane assignment](assign-lanes-benchmarks.md) | Assign the fewest lanes without overlaps within a lane. |
| [Weighted scheduling](weighted-scheduling-benchmarks.md) | Select a non-overlapping subset with maximum total weight. |
| [Capacity scheduling](capacity-scheduling-benchmarks.md) | Maximize total weight under an overlap limit. |
| [Minimum covering](covering-benchmarks.md) | Cover a target with the fewest intervals. |
| [Minimum-cost covering](cost-covering-benchmarks.md) | Cover a target at minimum total cost. |
| [Minimum stabbing points](stabbing-benchmarks.md) | Find the fewest points that hit every interval. |

## Reading the plots

Lines show medians and shaded bands show the sample range. Runtime axes are logarithmic.

## Run, regenerate, or extend

- [Running and publishing benchmarks](benchmarking.md): setup, measurement rules,
  chart configuration, and the checklist for a new operation.
- [Script inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md):
  what each Python script runs or summarizes.
- [Report template](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/report-template.md):
  a starting point for documenting a new operation.
