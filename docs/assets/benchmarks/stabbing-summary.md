**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Non-overlapping, sorted | 1,000 | 0.499 |
| Non-overlapping, sorted | 3,000,000 | 26.5 |
| Non-overlapping, shuffled | 1,000 | 0.21 |
| Non-overlapping, shuffled | 3,000,000 | 139 |
| Regular overlaps, sorted | 3,000,000 | 7.76 |
| Regular overlaps, shuffled | 3,000,000 | 115 |

[Exact values, sample ranges and counts](assets/benchmarks/stabbing-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-temporal-windows.csv).
