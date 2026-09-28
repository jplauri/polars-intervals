**Rust runtime (ms) · medians**

| Workload | Input rows | Packed greedy (production) | Indirect greedy | Heap |
| --- | ---: | ---: | ---: | ---: |
| Int64 touching chain | 1,000,000 | 50.1 | 84.2 | 54.8 |
| Int64 dense overlap | 1,000,000 | 47.5 | 65.4 | 64.2 |
| Int64 equal starts | 1,000,000 | 13.9 | 8.99 | 30.1 |
| Int64 mostly irrelevant | 1,000,000 | 0.91 | 0.934 | 0.883 |
| Date physical width, chain | 1,000,000 | 38.8 | 72.2 | 51.5 |

[Exact values, sample ranges and counts](assets/benchmarks/cover-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv).
