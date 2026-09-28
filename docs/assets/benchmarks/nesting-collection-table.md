**Collection time (ms), production · medians**

| Workload | Input rows | Int64 | Date | Datetime (µs) |
| --- | ---: | ---: | ---: | ---: |
| Depth 7, shuffled, 1 group | 100,000 | 5.12 | 4.5 | 5 |
| Depth 7, shuffled, 1 group | 3,000,000 | 203 | 170 | 200 |
| Full chain, shuffled, 1 group | 3,000,000 | 206 | 167 | 206 |
| Full chain, sorted, 1 group | 3,000,000 | 42.6 | — | — |
| Depth 7, shuffled, 1,000 groups | 100,000 | 10.9 | 11.5 | 12.1 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/nesting-collection-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv).
