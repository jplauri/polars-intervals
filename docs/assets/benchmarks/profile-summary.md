**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Constant capacity 4 | 1,000,000 | 77.6 |
| Enough capacity for all | 1,000,000 | 46.7 |
| Capacity varies: 2, 4, 6, 8 | 100,000 | 14.3 |
| Capacity varies: 2, 4, 6, 8 (larger input) | 1,000,000 | 160 |

[Exact values, sample ranges and counts](assets/benchmarks/profile-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-temporal.csv).
