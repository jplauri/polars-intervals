**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Depth up to 7, shuffled, 1 group | 100,000 | 5.12 |
| Depth up to 7, shuffled, 1 group | 3,000,000 | 203 |
| Fully nested, shuffled, 1 group | 3,000,000 | 206 |
| Fully nested, sorted, 1 group | 3,000,000 | 42.6 |
| Depth up to 7, shuffled, 1,000 groups | 100,000 | 10.9 |

[Exact values, sample ranges and counts](assets/benchmarks/nesting-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv).
