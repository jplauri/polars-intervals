**Complete lazy call, 24 threads (ms) · medians**

| Workload | Input rows | Package | Native Polars |
| --- | ---: | ---: | ---: |
| Strict clusters: 1,000 singleton timestamp groups | 1,000 | 10.9 | 7.4 |
| Strict clusters: shuffled overlapping chain | 100,000 | 4.74 | 15.3 |
| Strict clusters: shuffled overlapping chain | 1,000,000 | 61.3 | 136 |
| Union: 1,000 timestamp groups | 100,000 | 5.66 | 16.2 |
| Union: 1,000 timestamp groups | 1,000,000 | 50.7 | 83.6 |
| Gaps: 8 groups, narrow domain | 100,000 | 4.29 | 14.6 |
| Gaps: 8 groups, narrow domain | 1,000,000 | 38.7 | 83.7 |
| Union: 32 skewed groups | 1,000,000 | 57.1 | 106 |

[Exact values, sample ranges and counts](assets/benchmarks/interval-geometry-threads24-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-threads24-20260929.csv).
