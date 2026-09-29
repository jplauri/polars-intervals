**Polars query time (ms) · medians**

| Workload | Input rows | Ordinary assignment | Improve existing lanes | New balanced assignment |
| --- | ---: | ---: | ---: | ---: |
| 9 empty intervals among 2 overlapping ones | 11 | 0.0887 | 0.105 | 0.1 |
| Separate intervals followed by 8 overlapping ones | 100,000 | 0.911 | 3.17 | 3.76 |
| One long interval overlapping short ones | 100,000 | 1.16 | 3.4 | 9.41 |
| 50,000 long intervals overlapping short ones | 100,000 | 1.39 | 6.09 | 24.1 |

[Exact values, sample ranges and counts](assets/benchmarks/balance-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.timings.csv).
