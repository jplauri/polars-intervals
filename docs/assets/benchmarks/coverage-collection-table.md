**Polars query time (ms) · medians**

| Workload | Input rows | Integer | Date | Datetime |
| --- | ---: | ---: | ---: | ---: |
| Overlapping steps, sorted, select up to 8 | 10,000 | 1.08 | 1.01 | 1.12 |
| Overlapping steps, sorted, select up to 8 | 1,000,000 | 125 | 117 | 126 |
| Overlapping steps, shuffled, select up to 8 | 1,000,000 | 161 | 139 | 164 |
| Overlapping steps, shuffled, select up to 64 | 1,000,000 | 681 | 630 | 689 |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-collection-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-temporal.csv).
