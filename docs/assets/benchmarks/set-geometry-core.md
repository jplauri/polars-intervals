**Underlying algorithm time in milliseconds · medians**

| Workload | Total input rows | Union + scan (ms) | Fused runs (ms) | Events (ms) |
| --- | ---: | ---: | ---: | ---: |
| Subtract sorted availability | 1,000,000 | 17.9 | 20.5 | 120 |
| Subtract shuffled availability | 1,000,000 | 33.3 | 31.4 | 114 |
| Intersect shuffled availability | 1,000,000 | 31.6 | 26.3 | 109 |
| Intersect dense intervals | 1,000,000 | 32.2 | 33.4 | 120 |
| Intersect nested intervals | 1,000,000 | 17.5 | 21.1 | 111 |
| Subtract many holes from one domain | 1,000,000 | 45.7 | 41.5 | 111 |

[Exact values, sample ranges and counts](assets/benchmarks/set-geometry-core.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-core-million-20260930.csv).
