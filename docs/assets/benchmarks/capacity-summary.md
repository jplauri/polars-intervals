**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| 32 overlapping per set, capacity 1 | 1,000,000 | 28 |
| 32 overlapping per set, capacity 2 | 1,000,000 | 62.5 |
| 32 overlapping per set, capacity 31 | 1,000,000 | 119 |
| 32 overlapping per set, capacity 32 | 1,000,000 | 43.2 |

[Exact values, sample ranges and counts](assets/benchmarks/capacity-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv).
