**Complete call, including lazy plan construction (ms) · medians**

| Workload | Input rows | Eager | Lazy, auto engine | Lazy, streaming engine | Polars event sums |
| --- | ---: | ---: | ---: | ---: | ---: |
| Short intervals, start ordered, units | 8 | 0.0506 | 0.141 | 0.229 | 1.59 |
| Short intervals, start ordered, units | 1,000 | 0.0578 | 0.147 | 0.234 | 1.5 |
| Short intervals, start ordered, units | 100,000 | 1.24 | 1.58 | 1.71 | 41 |
| Variable durations, ordered, weighted | 100,000 | 8.7 | 9.11 | 9.41 | 79 |
| 32 resources, shuffled, clipped, weighted | 100,000 | 5.38 | 5.78 | 5.8 | 21 |
| 1,000 timestamp groups, shuffled, units | 100,000 | 5.83 | 6.31 | 6.38 | 53 |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-lazy-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.csv).
