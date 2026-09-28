**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| Depth up to 7, shuffled, 1 group | 100,000 | 5.12 | 4.5 | 5 |
| Depth up to 7, shuffled, 1 group | 3,000,000 | 203 | 170 | 200 |
| Fully nested, shuffled, 1 group | 3,000,000 | 206 | 167 | 206 |
| Fully nested, sorted, 1 group | 3,000,000 | 42.6 | — | — |
| Depth up to 7, shuffled, 1,000 groups | 100,000 | 10.9 | 11.5 | 12.1 |

— means no recorded measurement for that combination.

[Exact values, sample ranges and counts](assets/benchmarks/nesting-collection-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv).
