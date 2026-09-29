**Polars operation time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| Constant capacity 4 | 1,000,000 | 77.6 | 56.5 | 71 |
| Enough capacity for all | 1,000,000 | 46.7 | 31.2 | 43.3 |
| Capacity varies: 2, 4, 6, 8 | 100,000 | 14.3 | 13.1 | 13.9 |
| Capacity varies: 2, 4, 6, 8 (larger input) | 1,000,000 | 160 | 146 | 160 |

[Exact values, sample ranges and counts](assets/benchmarks/profile-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-temporal.csv).
