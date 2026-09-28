**Rust runtime (ms) · medians**

| Workload | Input rows | Production | Serial components | Whole flow | Generic flow |
| --- | ---: | ---: | ---: | ---: | ---: |
| Disjoint, k=2 | 1,000,000 | 85.6 | 86.4 | 443 | 2,450 |
| Dense, k=2 | 1,000,000 | 1,090 | 1,120 | 1,060 | 3,220 |
| Identical intervals, k=2 | 1,000,000 | 138 | 106 | 93.1 | 90.3 |
| Components, k=2 | 10,000 | 2.21 | 2.27 | 3.24 | 7.23 |
| Components, k=2 | 1,000,000 | 128 | 268 | 502 | 1,970 |
| Components, k=8 | 100,000 | 17.8 | 76.8 | 152 | 295 |
| Components, k=16 (sufficient) | 100,000 | 6.44 | 6.29 | 262 | 456 |

[Exact values, sample ranges and counts](assets/benchmarks/capacity-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv).
