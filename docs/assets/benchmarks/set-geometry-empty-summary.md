**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Subtract empty-only grouped rows | 100,000 | 8.93 | 7.14 | 1.3× slower |
| Subtract empty-only grouped rows | 1,000,000 | 84.7 | 18.7 | 4.5× slower |
| Intersect empty-only grouped rows | 100,000 | 10.2 | 8.14 | 1.3× slower |
| Intersect empty-only grouped rows | 1,000,000 | 85.6 | 18.4 | 4.7× slower |

[Exact values, sample ranges and counts](assets/benchmarks/set-geometry-empty-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20260930.csv).
