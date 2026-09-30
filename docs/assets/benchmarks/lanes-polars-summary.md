**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.212 |
| Non-overlapping, shuffled | 1,000,000 | 59.1 |
| Up to 128 overlapping, sorted by start | 1,000,000 | 24.4 |
| Up to 128 overlapping, shuffled | 1,000,000 | 121 |
| Nested, sorted by start | 1,000,000 | 30.4 |
| Nested, shuffled | 1,000,000 | 140 |
| Variable lengths, sorted by start | 1,000,000 | 80.4 |
| Variable lengths, shuffled | 1,000,000 | 176 |

[Exact values, sample ranges and counts](assets/benchmarks/lanes-polars-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.csv).
