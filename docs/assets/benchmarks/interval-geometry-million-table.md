**Complete lazy call, including planning (ms) · medians**

| Workload | Input rows | Package | Native Polars |
| --- | ---: | ---: | ---: |
| Strict clusters: ordered overlapping chain | 1,000,000 | 4.75 | 65.2 |
| Strict clusters: shuffled overlapping chain | 1,000,000 | 44.7 | 311 |
| Union: sorted nested intervals | 1,000,000 | 5.22 | 45.2 |
| Gaps: shuffled separate UInt64 intervals | 1,000,000 | 44.1 | 701 |
| Union: 1,000 timestamp groups | 1,000,000 | 58.3 | 259 |
| Gaps: 8 groups, narrow domain | 1,000,000 | 31.8 | 115 |

[Exact values, sample ranges and counts](assets/benchmarks/interval-geometry-million-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-million-20260929.csv).
