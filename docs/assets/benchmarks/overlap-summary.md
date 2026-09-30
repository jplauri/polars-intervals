**Times in milliseconds · medians**

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Few overlaps, 1 group | 100,000 | 6.15 | 10.5 | 1.7× faster |
| Few overlaps, 1 group | 3,000,000 | 244 | 339 | 1.4× faster |
| Many overlaps, 1 group | 3,000,000 | 176 | 339 | 1.9× faster |
| Few overlaps, 100 groups | 1,000 | 1.06 | 1.04 | about the same |
| Few overlaps, 100 groups | 3,000,000 | 214 | 384 | 1.8× faster |
| Many overlaps, 100 groups | 1,000 | 1.14 | 1.04 | about the same |
| Many overlaps, 100 groups | 3,000,000 | 167 | 376 | 2.3× faster |

[Exact values, sample ranges and counts](assets/benchmarks/overlap-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).
