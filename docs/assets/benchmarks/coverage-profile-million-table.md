**Complete eager call (ms) · medians**

| Workload | Input rows | Package | Polars event sums | Polars event counts |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, shuffled, units | 1,000,000 | 45.5 | 1,230 | 1,510 |
| All overlap, shuffled, weighted | 1,000,000 | 24.3 | 66 | — |
| Repeated dates, shuffled, units | 1,000,000 | 26.1 | 118 | 90.1 |
| 1,000 timestamp groups, shuffled, units | 1,000,000 | 77.5 | 988 | 1,290 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-million-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-million-20260929.csv).
