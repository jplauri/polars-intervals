**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| End-to-end intervals, shuffled | 1,000,000 | 69 | 39.8 | 52.6 |
| Many overlaps, shuffled | 1,000,000 | 51 | 36.2 | 49.4 |

[Exact values, sample ranges and counts](assets/benchmarks/cover-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv).
