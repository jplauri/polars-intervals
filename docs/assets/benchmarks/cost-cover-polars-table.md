**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| End-to-end intervals, shuffled | 1,000,000 | 211 | 196 | 206 |
| Many overlaps, shuffled | 1,000,000 | 263 | 238 | 264 |

[Exact values, sample ranges and counts](assets/benchmarks/cost-cover-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-temporal-windows.csv).
