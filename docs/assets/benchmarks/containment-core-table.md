**Algorithm time (ms) · medians**

| Workload | Input rows | Package design (packed Fenwick) | Indirect Fenwick | Segment tree |
| --- | ---: | ---: | ---: | ---: |
| Repeated intervals | 3,000,000 | 53.2 | 38.6 | 53.8 |
| Few containments | 3,000,000 | 1,010 | 1,340 | 1,120 |
| Many containments | 3,000,000 | 418 | 779 | 488 |

[Exact values, sample ranges and counts](assets/benchmarks/containment-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv).
