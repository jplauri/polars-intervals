**Times in milliseconds · medians**

| Workload | Queries n = sources m | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Plan construction only | 100,000 | 0.21 | 0.909 | 4.3× faster |
| Collect a prebuilt plan | 100,000 | 25.6 | 281 | 11× faster |
| Eager queries and lazy sources | 100,000 | 24.3 | 268 | 11× faster |
| Streaming-engine complete call | 100,000 | 27.7 | 325 | 12× faster |
| Warm Parquet prebuilt-plan collection | 100,000 | 33.4 | 313 | 9.4× faster |
| Warm Parquet streaming complete call | 100,000 | 35 | 356 | 10× faster |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-stats-modes.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-modes.csv).
