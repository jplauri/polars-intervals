**Complete Rust call (ms) · medians**

| Workload | Input rows | B: package streams | A: events | C: active-end heap | B: weighted indices |
| --- | ---: | ---: | ---: | ---: | ---: |
| Short intervals, ordered, units | 1,000,000 | 44.5 | 88.8 | 46.7 | — |
| Nested, ordered, units | 1,000,000 | 44.5 | 93.1 | 121 | — |
| Variable durations, ordered, weighted | 1,000,000 | 94.1 | 104 | 69.4 | 75 |
| Variable durations, shuffled, weighted | 1,000,000 | 131 | 135 | 213 | 254 |
| Repeated endpoints, shuffled, weighted | 100,000 | 3.22 | 3.05 | 8.9 | 2.59 |
| Repeated endpoints, shuffled, weighted | 1,000,000 | 48.6 | 55.1 | 217 | 78.7 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-core-final-20260929.csv).
