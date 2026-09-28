**Runtime (ms) · medians**

| Workload | Input rows | Two sorted orders (production design) | Binary search | Endpoint sweep |
| --- | ---: | ---: | ---: | ---: |
| Disjoint, shuffled | 1,000 | 0.0358 | 0.0478 | 0.0681 |
| Disjoint, shuffled | 1,000,000 | 81.9 | 148 | 204 |
| Moderate overlap, finish-sorted | 1,000,000 | 20.9 | 46.3 | 109 |
| Nested, shuffled | 1,000,000 | 78.1 | 133 | 138 |
| Random lengths, finish-sorted | 1,000,000 | 120 | 82.8 | 118 |
| Random lengths, shuffled | 1,000,000 | 240 | 199 | 153 |

[Exact values, sample ranges and counts](assets/benchmarks/weighted-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv).
