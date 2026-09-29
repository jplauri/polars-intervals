**Polars query time (ms) · medians**

| Workload | Input rows | containment_count (measured build) | Polars ranking expressions |
| --- | ---: | ---: | ---: |
| Repeated intervals, no grouping | 3,000,000 | 65.3 | 146 |
| Few containments, no grouping | 3,000,000 | 1,090 | 2,010 |
| Mixed intervals, no grouping | 3,000,000 | 363 | 459 |
| Few containments, 1,000 groups | 10,000 | 8.76 | 6.75 |

[Exact values, sample ranges and counts](assets/benchmarks/containment-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json).
