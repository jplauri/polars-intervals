**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm (sort records) | Sort row indices | Heap alternative |
| --- | ---: | ---: | ---: | ---: |
| 64-bit endpoints, end-to-end intervals | 1,000,000 | 50.1 | 84.2 | 54.8 |
| 64-bit endpoints, many overlaps | 1,000,000 | 47.5 | 65.4 | 64.2 |
| 64-bit endpoints, same start | 1,000,000 | 13.9 | 8.99 | 30.1 |
| 64-bit endpoints, mostly outside target | 1,000,000 | 0.91 | 0.934 | 0.883 |
| 32-bit endpoints, end-to-end intervals | 1,000,000 | 38.8 | 72.2 | 51.5 |

[Exact values, sample ranges and counts](assets/benchmarks/cover-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv).
