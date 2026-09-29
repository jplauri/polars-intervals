**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm (Fenwick tree) | Segment tree |
| --- | ---: | ---: | ---: |
| 64-bit endpoints, end-to-end intervals | 1,000,000 | 188 | 205 |
| 64-bit endpoints, many overlaps | 1,000,000 | 226 | 390 |
| 64-bit endpoints, same start | 1,000,000 | 199 | 274 |
| 64-bit endpoints, mostly outside target | 1,000,000 | 2.05 | 2.04 |
| 32-bit endpoints, end-to-end intervals | 1,000,000 | 211 | 196 |

[Exact values, sample ranges and counts](assets/benchmarks/cost-cover-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv).
