**Core call time (ms) · medians**

| Workload | Input rows | Production | Reduction + cover | Fused cover | Heap DP |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shuffled disjoint, positive costs | 100,000 | 14.8 | 23.4 | 18.6 | 14.5 |
| Shuffled path, mixed costs | 100,000 | 16.4 | 23.8 | 19.6 | 16.4 |
| Clique, shuffled, positive costs | 100,000 | 15.5 | 12.8 | 14.2 | 15.4 |
| Nested, end sorted, positive costs | 100,000 | 5.43 | 4.36 | 2.82 | 5.6 |
| Duplicate geometries, reversed, units | 100,000 | 5.72 | 8.97 | 4.94 | 9.3 |
| Path, start sorted, units | 1,000 | 0.0056 | 0.0357 | 0.0314 | 0.0545 |
| Path, start sorted, units | 100,000 | 0.685 | 9 | 7.54 | 9.99 |
| Shuffled path, explicit zero costs | 100,000 | 7.78 | 23.9 | 21.6 | 14.9 |

[Exact values, sample ranges and counts](assets/benchmarks/domination-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-core-production-windows-20260929.csv).
