**Times in milliseconds · medians**

| Workload | Queries n = sources m | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Narrow queries and sources | 100,000 | 20.6 | 275 | 13× faster |
| 16 string columns and nested query payloads, wide sources | 100,000 | 24.6 | 1,550 | 63× faster |
| Narrow queries, 16 irrelevant source columns | 100,000 | 31.5 | 267 | 8.5× faster |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-stats-payloads.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-local.csv).
