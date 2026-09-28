**Runtime (ms) · medians**

| Workload | Input rows | Production | Rolling rows reference | Component decomposition |
| --- | ---: | ---: | ---: | ---: |
| Staircase, shuffled, k=32 | 10,000 | 1.71 | 2.45 | 4.66 |
| Components, sorted, k=8 | 10,000 | 0.475 | 0.671 | 1.11 |
| Components, sorted, k=64 | 10,000 | 3.1 | 4.71 | 2.65 |
| Saturation, shuffled, k=64 | 100,000 | 4.29 | 57.4 | — |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-core.csv).
