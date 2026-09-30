**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Label overlapping intervals, shuffled | 100,000 | 4.74 | 15.3 | 3.2× faster |
| Label overlapping intervals, shuffled | 1,000,000 | 61.3 | 136 | 2.2× faster |
| Label intervals in 1,000 timestamp groups, shuffled | 1,000 | 10.9 | 7.4 | 1.5× slower |
| Label intervals in 1,000 timestamp groups, shuffled | 100,000 | 15.2 | 18.1 | 1.2× faster |
| Merge intervals in 1,000 timestamp groups, shuffled | 100,000 | 5.66 | 16.2 | 2.9× faster |
| Merge intervals in 1,000 timestamp groups, shuffled | 1,000,000 | 50.7 | 83.6 | 1.6× faster |
| Find gaps in 8 groups within a narrow domain, shuffled | 100,000 | 4.29 | 14.6 | 3.4× faster |
| Find gaps in 8 groups within a narrow domain, shuffled | 1,000,000 | 38.7 | 83.7 | 2.2× faster |
| Merge intervals in 32 uneven groups, shuffled | 1,000,000 | 57.1 | 106 | 1.9× faster |

[Exact values, sample ranges and counts](assets/benchmarks/geometry-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.csv).
