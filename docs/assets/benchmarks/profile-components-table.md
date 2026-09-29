**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm | Always use one worker | Always use eight workers |
| --- | ---: | ---: | ---: | ---: |
| 32 intervals per overlap set, 16 capacity segments | 1,000 | 0.62 | 0.648 | 0.714 |
| 32 intervals per overlap set, 16 capacity segments | 10,000 | 5.74 | 6.02 | 2.25 |
| 32 intervals per overlap set, 16 capacity segments | 1,000,000 | 226 | 671 | 217 |

[Exact values, sample ranges and counts](assets/benchmarks/profile-components-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-core.csv).
