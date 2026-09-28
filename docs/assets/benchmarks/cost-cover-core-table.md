**Rust runtime (ms) · medians**

| Workload | Input rows | Fenwick (production) | Segment tree |
| --- | ---: | ---: | ---: |
| Int64 touching chain | 1,000,000 | 188 | 205 |
| Int64 dense overlap | 1,000,000 | 226 | 390 |
| Int64 equal starts | 1,000,000 | 199 | 274 |
| Int64 mostly irrelevant | 1,000,000 | 2.05 | 2.04 |
| Date physical width, chain | 1,000,000 | 211 | 196 |

[Exact values, sample ranges and counts](assets/benchmarks/cost-cover-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv).
