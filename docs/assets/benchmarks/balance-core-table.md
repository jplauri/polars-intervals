**Core call time (ms) · medians**

| Workload | Input rows | Existing assignment | Best seed only | Repair baseline | Balanced constructor |
| --- | ---: | ---: | ---: | ---: | ---: |
| Long/short, sorted | 10,000 | 0.087 | 0.673 | 0.293 | 0.869 |
| Long/short, sorted | 100,000 | 0.609 | 7.7 | 2.59 | 9.36 |
| Mixed empties, shuffled | 100,000 | 3.62 | 9.46 | 8.01 | 9.56 |
| Stars, sorted | 1,000 | 0.0056 | 0.06 | 0.0242 | 0.0727 |
| Clique, shuffled | 100,000 | 0.925 | 2.02 | 2.18 | 1.39 |

[Exact values, sample ranges and counts](assets/benchmarks/balance-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-core-windows-20260928.default-timings.csv).
