**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Shuffled availability and busy windows | 1,000 | 1.18 | 6.98 | 5.9× faster |
| Shuffled availability and busy windows | 100,000 | 9.02 | 51.6 | 5.7× faster |
| Shuffled availability and busy windows | 1,000,000 | 83.8 | 509 | 6.1× faster |
| Many tiny timestamp groups | 1,000 | 1.79 | 15 | 8.4× faster |
| Nested intervals | 1,000,000 | 55.7 | 206 | 3.7× faster |

[Exact values, sample ranges and counts](assets/benchmarks/subtract-intervals-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20260930.csv).
