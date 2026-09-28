**Algorithm time (ms) · medians**

| Workload | Input rows | Package algorithm | Endpoint scan | Packed endpoint lists | Active-interval heap |
| --- | ---: | ---: | ---: | ---: | ---: |
| Up to 64 overlapping, shuffled, no weights | 100,000 | 3.16 | 6.52 | 3.39 | 5.74 |
| Up to 64 overlapping, shuffled, no weights | 1,000,000 | 40.6 | 75.5 | 42.1 | 119 |
| Up to 2 overlapping, shuffled, 128-bit weights | 100,000 | 6.56 | 7.12 | 7.91 | 4.58 |
| Up to 2 overlapping, shuffled, 128-bit weights | 1,000,000 | 116 | 112 | 99.3 | 122 |
| Repeated endpoints, shuffled, 128-bit weights | 1,000,000 | 49.3 | 53.3 | 47.5 | 130 |
| All overlap, shuffled, 128-bit weights | 1,000,000 | 6.04 | 125 | 97 | 103 |
| Non-overlapping, shuffled, 64-bit weights | 100,000 | 6.67 | 7.23 | 5.34 | 4.43 |

[Exact values, sample ranges and counts](assets/benchmarks/clique-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-windows-20260928.csv).
