**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Shuffled availability and busy windows | 1,000 | 0.522 | 5.65 | 11× faster |
| Shuffled availability and busy windows | 100,000 | 4.73 | 35 | 7.4× faster |
| Shuffled availability and busy windows | 1,000,000 | 37.5 | 324 | 8.6× faster |
| Many tiny timestamp groups | 1,000 | 1.1 | 11.3 | 10× faster |
| Nested intervals | 1,000,000 | 21.3 | 198 | 9.3× faster |

[Exact values, sample ranges and counts](assets/benchmarks/intersect-intervals-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20261001.csv).
