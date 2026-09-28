**Runtime (ms) · medians**

| Workload | Input rows | Packed Fenwick (production design) | Indirect Fenwick | Segment tree |
| --- | ---: | ---: | ---: | ---: |
| Duplicates | 3,000,000 | 53.2 | 38.6 | 53.8 |
| Sparse | 3,000,000 | 1,010 | 1,340 | 1,120 |
| Dense | 3,000,000 | 418 | 779 | 488 |

[Exact values, sample ranges and counts](assets/benchmarks/containment-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/containment-kernels-windows.csv).
