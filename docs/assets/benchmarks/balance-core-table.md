**Algorithm time (ms) · medians**

| Workload | Input rows | Ordinary assignment | Initial choices only | Improve existing lanes | New balanced assignment |
| --- | ---: | ---: | ---: | ---: | ---: |
| Mixed long and short intervals, sorted | 10,000 | 0.087 | 0.673 | 0.293 | 0.869 |
| Mixed long and short intervals, sorted | 100,000 | 0.609 | 7.7 | 2.59 | 9.36 |
| Includes empty intervals, shuffled | 100,000 | 3.62 | 9.46 | 8.01 | 9.56 |
| Long intervals spanning short ones, sorted | 1,000 | 0.0056 | 0.06 | 0.0242 | 0.0727 |
| All overlap, shuffled | 100,000 | 0.925 | 2.02 | 2.18 | 1.39 |

[Exact values, sample ranges and counts](assets/benchmarks/balance-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-core-windows-20260928.default-timings.csv).
