**Polars query time (ms) · medians**

| Workload | Input rows | No weights | All weights = 1 | Positive weights | Positive and negative weights |
| --- | ---: | ---: | ---: | ---: | ---: |
| Up to 8 overlapping, shuffled integers | 1,000 | 0.0803 | 0.0985 | 0.0972 | 0.0788 |
| Up to 8 overlapping, shuffled integers | 100,000 | 3.87 | 7.38 | 7.45 | 3.26 |
| Up to 8 overlapping, shuffled integers | 1,000,000 | 43.1 | 148 | 140 | 45.5 |
| All overlap, shuffled dates | 1,000,000 | 2.06 | 11 | 10.6 | 12.6 |
| Up to 8 overlapping, shuffled UTC datetimes, streaming | 1,000,000 | 48.2 | 129 | 126 | 48.6 |
| 32 groups, shuffled integers, multiple chunks | 1,000 | 0.414 | 0.409 | 0.437 | 0.433 |
| 32 groups, shuffled integers, multiple chunks | 1,000,000 | 49.6 | 82.9 | 83.1 | 44.7 |

[Exact values, sample ranges and counts](assets/benchmarks/clique-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.timings.csv).
