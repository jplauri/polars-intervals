**Polars collection time (ms) · medians**

| Workload | Input rows | Existing assignment | Repair baseline | Balanced constructor |
| --- | ---: | ---: | ---: | ---: |
| Interior empties | 11 | 0.0887 | 0.105 | 0.1 |
| Late clique, k=8 | 100,000 | 0.911 | 3.17 | 3.76 |
| Forced star, k=2 | 100,000 | 1.16 | 3.4 | 9.41 |
| Nearly clique, k=50,001 | 100,000 | 1.39 | 6.09 | 24.1 |

[Exact values, sample ranges and counts](assets/benchmarks/balance-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.timings.csv).
