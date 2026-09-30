**Times in milliseconds · medians**

| Workload | Rows | No cost column (ms) | Varying costs (ms) |
| --- | ---: | ---: | ---: |
| Chain, shuffled integers | 1,000 | 0.115 | 0.179 |
| Chain, shuffled integers | 100,000 | 7.97 | 18 |
| 32 chains, shuffled integers, multiple chunks | 1,000 | 0.385 | 0.502 |
| 32 chains, shuffled integers, multiple chunks | 100,000 | 7.94 | 14.3 |

[Exact values, sample ranges and counts](assets/benchmarks/domination-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-polars-windows-20260929.timings.csv).
