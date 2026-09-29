**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm (heap) | Two sorted lists | Endpoint scan |
| --- | ---: | ---: | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.0216 | 0.0445 | 0.0617 |
| Non-overlapping, shuffled | 1,000,000 | 79.7 | 137 | 101 |
| Up to 128 overlapping, sorted | 1,000,000 | 23.5 | 10.2 | 87.9 |
| Up to 128 overlapping, shuffled | 1,000,000 | 153 | 140 | 100 |
| Nested, sorted | 1,000,000 | 29.9 | 9.08 | 85.6 |
| Nested, shuffled | 1,000,000 | 172 | 121 | 96 |

[Exact values, sample ranges and counts](assets/benchmarks/lanes-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv).
