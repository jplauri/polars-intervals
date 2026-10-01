**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Shuffled availability and busy windows | 1,000 | 0.495 | 6.05 | 12× faster |
| Shuffled availability and busy windows | 100,000 | 5.16 | 38.2 | 7.4× faster |
| Shuffled availability and busy windows | 1,000,000 | 40.6 | 406 | 10× faster |
| Many tiny timestamp groups | 1,000 | 1.15 | 10.7 | 9.3× faster |
| Nested intervals | 1,000,000 | 21.6 | 169 | 7.8× faster |

[Exact values, sample ranges and counts](assets/benchmarks/subtract-intervals-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-repeat-20261001.csv).
