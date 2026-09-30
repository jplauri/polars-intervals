**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Label overlapping intervals, shuffled | 1,000 | 0.148 | 0.864 | 5.8× faster |
| Label overlapping intervals, shuffled | 100,000 | 4.11 | 30.8 | 7.5× faster |
| Label touching intervals, reversed | 100,000 | 1.64 | 6.89 | 4.2× faster |
| Merge separate intervals, shuffled | 100,000 | 4.16 | 42 | 10× faster |
| Find gaps between separate intervals, shuffled | 100,000 | 4.28 | 44.2 | 10× faster |
| Label intervals in 1,000 timestamp groups, shuffled | 1,000 | 10.3 | 4.73 | 2.2× slower |
| Label intervals in 1,000 timestamp groups, shuffled | 100,000 | 14.3 | 26.4 | 1.8× faster |
| Find gaps in 32 empty-only groups | 100,000 | 1.91 | 6.34 | 3.3× faster |

[Exact values, sample ranges and counts](assets/benchmarks/geometry-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.csv).
