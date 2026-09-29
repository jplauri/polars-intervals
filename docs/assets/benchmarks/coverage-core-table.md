**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm | Simpler optimization reference | Solve overlap sets separately |
| --- | ---: | ---: | ---: | ---: |
| Overlapping steps, shuffled, select up to 32 | 10,000 | 1.71 | 2.45 | 4.66 |
| Independent overlap sets, sorted, select up to 8 | 10,000 | 0.475 | 0.671 | 1.11 |
| Independent overlap sets, sorted, select up to 64 | 10,000 | 3.1 | 4.71 | 2.65 |
| Limit covers full range, shuffled, select up to 64 | 100,000 | 4.29 | 57.4 | — |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-core.csv).
