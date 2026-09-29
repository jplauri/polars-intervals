**Complete eager call (ms) · medians**

| Workload | Input rows | Package | Polars event sums | Polars event counts |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, start ordered, units | 8 | 0.0642 | 1.38 | 1.44 |
| Short intervals, start ordered, units | 1,000 | 0.0753 | 1.6 | 1.67 |
| Short intervals, start ordered, units | 100,000 | 1.46 | 36.5 | 51.3 |
| Short intervals, shuffled, units | 100,000 | 4.45 | 48.5 | 65.8 |
| Variable durations, ordered, weighted | 100,000 | 7.68 | 73 | — |
| 32 resources, shuffled, clipped, weighted | 100,000 | 5.42 | 18.8 | — |
| 1,000 timestamp groups, shuffled, units | 100,000 | 6.38 | 55.2 | 60.8 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-polars-20260929.csv).
