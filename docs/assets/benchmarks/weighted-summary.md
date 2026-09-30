**Times in milliseconds · medians**

| Workload | Rows | Package algorithm (ms) |
| --- | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.0358 |
| Non-overlapping, shuffled | 1,000,000 | 81.9 |
| Moderate overlap, sorted by end | 1,000,000 | 20.9 |
| Nested, shuffled | 1,000,000 | 78.1 |
| Random lengths, sorted by end | 1,000,000 | 120 |
| Random lengths, shuffled | 1,000,000 | 240 |

[Exact values, sample ranges and counts](assets/benchmarks/weighted-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv).
