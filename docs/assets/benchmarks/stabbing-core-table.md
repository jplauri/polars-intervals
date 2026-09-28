**Rust runtime (ms) · medians**

| Workload | Input rows | Production | Packed sort | Indirect sort |
| --- | ---: | ---: | ---: | ---: |
| Disjoint, sorted | 3,000,000 | 17.8 | 31.4 | 24.6 |
| Disjoint, shuffled | 3,000,000 | 113 | 109 | 283 |
| Disjoint, reverse | 3,000,000 | 35.2 | 35.4 | 26.8 |
| Common intersection, reverse | 3,000,000 | 23.7 | 23.7 | 14.4 |
| Dense random, shuffled | 3,000,000 | 98.3 | 99.3 | 230 |

[Exact values, sample ranges and counts](assets/benchmarks/stabbing-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-windows.csv).
