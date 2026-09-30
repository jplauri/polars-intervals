**Polars call time (ms) · medians**

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, sorted | 1,000 | 0.0578 | 1.5 | 26× faster |
| Short intervals, sorted | 100,000 | 1.24 | 41 | 33× faster |
| Variable durations, sorted, weighted | 100,000 | 8.7 | 79 | 9.1× faster |
| 32 resources, shuffled, limited time range, weighted | 100,000 | 5.38 | 21 | 3.9× faster |
| 1,000 timestamp groups, shuffled | 100,000 | 5.83 | 53 | 9.1× faster |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-profile-headline.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-profile-ponytail-20260929.csv).
