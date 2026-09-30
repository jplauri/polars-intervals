**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Repeated intervals, no grouping | 3,000,000 | 65.3 | 146 | 2.2× faster |
| Few containments, no grouping | 3,000,000 | 1,090 | 2,010 | 1.8× faster |
| Mixed intervals, no grouping | 3,000,000 | 363 | 459 | 1.3× faster |
| Few containments, 1,000 groups | 10,000 | 8.76 | 6.75 | 1.3× slower |

[Exact values, sample ranges and counts](assets/benchmarks/containment-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-polars-rank-runs-windows.json).
