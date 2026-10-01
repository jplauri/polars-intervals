**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Shuffled availability and busy windows | 1,000 | 1.15 | 8.8 | 7.7× faster |
| Shuffled availability and busy windows | 100,000 | 8.08 | 37.8 | 4.7× faster |
| Shuffled availability and busy windows | 1,000,000 | 81 | 394 | 4.9× faster |
| Many tiny timestamp groups | 1,000 | 1.94 | 19.9 | 10× faster |
| Nested intervals | 1,000,000 | 62.4 | 295 | 4.7× faster |

[Exact values, sample ranges and counts](assets/benchmarks/intersect-intervals-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20260930.csv).
