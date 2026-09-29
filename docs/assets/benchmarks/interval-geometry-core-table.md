**Complete Rust call (ms) · medians**

| Workload | Input rows | Package | Packed records | Indices | Union then gaps |
| --- | ---: | ---: | ---: | ---: | ---: |
| Strict clusters: sorted disjoint | 1,000,000 | 3.66 | 20.9 | 13.9 | — |
| Strict clusters: shuffled disjoint | 1,000,000 | 53.4 | 57.8 | 69.3 | — |
| Strict clusters: partly ordered spanning rows | 1,000,000 | 38.1 | 43.5 | 31.4 | — |
| Union: sorted nested | 1,000,000 | 3.91 | 11.5 | 8.76 | — |
| Gaps: sorted nested | 1,000,000 | 3.05 | 10.5 | 7.77 | 11.5 |
| Gaps: shuffled disjoint | 1,000,000 | 38.6 | 38.4 | 65.6 | 45.8 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/interval-geometry-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/interval-geometry-core-million-20260929.csv).
