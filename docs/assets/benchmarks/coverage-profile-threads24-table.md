**Complete eager call (ms) · medians**

| Workload | Input rows | Package | Polars event sums | Polars event counts |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, shuffled, units | 1,000,000 | 50.9 | 237 | 353 |
| All overlap, shuffled, weighted | 1,000,000 | 30 | 66.2 | — |
| Repeated dates, shuffled, units | 1,000,000 | 29.7 | 68.5 | 58.6 |
| 32 resources, shuffled, clipped, weighted | 1,000,000 | 95.6 | 120 | — |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-threads24-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-threads24-20260929.csv).
