**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| Non-overlapping, sorted | 3,000,000 | 26.5 | 15 | 27.6 |
| Non-overlapping, shuffled | 3,000,000 | 139 | 96.6 | 129 |
| Regular overlaps, sorted | 3,000,000 | 7.76 | 4.41 | 6.45 |
| Regular overlaps, shuffled | 3,000,000 | 115 | 78.7 | 114 |

[Exact values, sample ranges and counts](assets/benchmarks/stabbing-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-temporal-windows.csv).
