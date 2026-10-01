**Times in milliseconds · medians**

| Workload | Varying input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| 8 queries, source rows shown | 1,000,000 | 23.8 | 81.6 | 3.4× faster |
| 8 sources, query rows shown | 1,000,000 | 74.3 | 2,010 | 27× faster |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-stats-ratios.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-million.csv).
