**Collection time (ms) · medians**

| Workload | Input rows | Plugin (production at recorded revision) | Native run rank |
| --- | ---: | ---: | ---: |
| Duplicates, global Int64 | 3,000,000 | 65.3 | 146 |
| Sparse, global Int64 | 3,000,000 | 1,090 | 2,010 |
| Mixed, global Int64 | 3,000,000 | 363 | 459 |
| Sparse, 1,000 groups, Int64 | 10,000 | 8.76 | 6.75 |

[Exact values, sample ranges and counts](assets/benchmarks/containment-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json).
