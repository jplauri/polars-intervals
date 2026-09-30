**Times in milliseconds · medians**

| Workload | Rows | No weights (ms) | Positive weights (ms) |
| --- | ---: | ---: | ---: |
| Up to 8 overlapping, shuffled integers | 1,000 | 0.0803 | 0.0972 |
| Up to 8 overlapping, shuffled integers | 100,000 | 3.87 | 7.45 |
| Up to 8 overlapping, shuffled integers | 1,000,000 | 43.1 | 140 |
| All overlap, shuffled dates | 1,000,000 | 2.06 | 10.6 |
| Up to 8 overlapping, shuffled UTC datetimes, streaming | 1,000,000 | 48.2 | 126 |
| 32 groups, shuffled integers, multiple chunks | 1,000 | 0.414 | 0.437 |
| 32 groups, shuffled integers, multiple chunks | 1,000,000 | 49.6 | 83.1 |

[Exact values, sample ranges and counts](assets/benchmarks/clique-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.timings.csv).
