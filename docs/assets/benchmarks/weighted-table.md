**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm | Binary search | Endpoint scan |
| --- | ---: | ---: | ---: | ---: |
| Non-overlapping, shuffled | 1,000 | 0.0358 | 0.0478 | 0.0681 |
| Non-overlapping, shuffled | 1,000,000 | 81.9 | 148 | 204 |
| Moderate overlap, sorted by end | 1,000,000 | 20.9 | 46.3 | 109 |
| Nested, shuffled | 1,000,000 | 78.1 | 133 | 138 |
| Random lengths, sorted by end | 1,000,000 | 120 | 82.8 | 118 |
| Random lengths, shuffled | 1,000,000 | 240 | 199 | 153 |

[Exact values, sample ranges and counts](assets/benchmarks/weighted-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv).
