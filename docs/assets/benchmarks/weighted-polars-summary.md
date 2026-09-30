**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.256 |
| Non-overlapping, shuffled | 1,000,000 | 96.7 |
| Up to 128 overlapping, sorted by start | 1,000,000 | 25.3 |
| Up to 128 overlapping, shuffled | 1,000,000 | 96 |
| Nested, sorted by start | 1,000,000 | 26.2 |
| Nested, shuffled | 1,000,000 | 96.9 |
| Variable lengths, sorted by start | 1,000,000 | 183 |
| Variable lengths, shuffled | 1,000,000 | 260 |

[Exact values, sample ranges and counts](assets/benchmarks/weighted-polars-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/scheduling-polars-20260930.csv).
