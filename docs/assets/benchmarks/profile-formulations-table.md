**Rust runtime (ms) · medians**

| Workload | Input rows | Production | Forced transshipment | Forced circulation |
| --- | ---: | ---: | ---: | ---: |
| Moderate overlap, 1K profile segments | 10,000 | 37.9 | 386 | 39.5 |
| Moderate overlap, 10K profile segments | 10,000 | 37.3 | 3,820 | 40.3 |

[Exact values, sample ranges and counts](assets/benchmarks/profile-formulations-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-core.csv).
