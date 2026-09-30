**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) |
| --- | ---: | ---: |
| Overlapping steps, sorted, select up to 8 | 10,000 | 1.08 |
| Overlapping steps, sorted, select up to 8 | 1,000,000 | 125 |
| Overlapping steps, shuffled, select up to 8 | 1,000,000 | 161 |
| Overlapping steps, shuffled, select up to 64 | 1,000,000 | 681 |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv).
