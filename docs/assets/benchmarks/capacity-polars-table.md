**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| 32 overlapping per set, capacity 1 | 1,000,000 | 28 | 44.2 | 28.3 |
| 32 overlapping per set, capacity 2 | 1,000,000 | 62.5 | 49.1 | 83.3 |
| 32 overlapping per set, capacity 31 | 1,000,000 | 119 | 101 | 126 |
| 32 overlapping per set, capacity 32 | 1,000,000 | 43.2 | 31.7 | 41 |

[Exact values, sample ranges and counts](assets/benchmarks/capacity-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv).
