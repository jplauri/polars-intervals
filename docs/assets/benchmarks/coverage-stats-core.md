**Underlying algorithm time in milliseconds · medians**

| Workload | Queries n | Production (ms) | Binary searches (ms) | Packed sweep (ms) |
| --- | ---: | ---: | ---: | ---: |
| Sorted disjoint, m = 1M | 1,000,000 | 58.4 | 195 | 81 |
| Sorted genomic, m = 1M | 1,000,000 | 93.8 | 145 | 111 |
| Shuffled genomic, retained loss, m = 1M | 1,000,000 | 400 | 420 | 271 |
| Duplicated sources, m = 1M | 1,000,000 | 227 | 477 | 229 |
| Sparse coordinates, m = 1M | 1,000,000 | 263 | 1,040 | 263 |
| One union run, m = 1M | 1,000,000 | 145 | 146 | 252 |
| Query-heavy genomic, m = 10k | 1,000,000 | 76.1 | 75.2 | 201 |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-stats-core.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-core-production.csv).
