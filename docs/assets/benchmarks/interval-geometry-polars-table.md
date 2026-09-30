**Complete eager call (ms) · medians**

| Workload | Input rows | Package | Native Polars |
| --- | ---: | ---: | ---: |
| Strict clusters: shuffled overlapping chain | 8 | 0.113 | 0.709 |
| Strict clusters: shuffled overlapping chain | 1,000 | 0.148 | 0.864 |
| Strict clusters: shuffled overlapping chain | 100,000 | 4.11 | 30.8 |
| Touching clusters: reversed touching chain | 100,000 | 1.64 | 6.89 |
| Union: shuffled separate UInt64 intervals | 100,000 | 4.16 | 42 |
| Gaps: shuffled separate UInt64 intervals | 100,000 | 4.28 | 44.2 |
| Strict clusters: 1,000 timestamp groups | 1,000 | 10.3 | 4.73 |
| Strict clusters: 1,000 timestamp groups | 100,000 | 14.3 | 26.4 |
| Gaps: 32 empty-only observed groups | 100,000 | 1.91 | 6.34 |

[Exact values, sample ranges and counts](assets/benchmarks/interval-geometry-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-polars-20260929.csv).
