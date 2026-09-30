**Times in milliseconds · medians**

| Workload | Rows | Package algorithm (ms) |
| --- | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.0216 |
| Non-overlapping, shuffled | 1,000,000 | 79.7 |
| Up to 128 overlapping, sorted | 1,000,000 | 23.5 |
| Up to 128 overlapping, shuffled | 1,000,000 | 153 |
| Nested, sorted | 1,000,000 | 29.9 |
| Nested, shuffled | 1,000,000 | 172 |

[Exact values, sample ranges and counts](assets/benchmarks/lanes-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv).
