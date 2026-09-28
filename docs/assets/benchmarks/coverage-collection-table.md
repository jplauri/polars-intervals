**Collection time (ms), production · medians**

| Workload | Input rows | Int64 | Date | Datetime (µs) |
| --- | ---: | ---: | ---: | ---: |
| Staircase, sorted, k=8 | 10,000 | 1.08 | 1.01 | 1.12 |
| Staircase, sorted, k=8 | 1,000,000 | 125 | 117 | 126 |
| Staircase, shuffled, k=8 | 1,000,000 | 161 | 139 | 164 |
| Staircase, shuffled, k=64 | 1,000,000 | 681 | 630 | 689 |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-collection-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv).
