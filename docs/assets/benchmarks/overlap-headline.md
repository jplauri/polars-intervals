**Collection time (ms) · medians**

| Workload | Input rows | Endpoint sweep (production) | Search sorted | As-of joins |
| --- | ---: | ---: | ---: | ---: |
| Sparse, 1 group | 100,000 | 6.15 | 10.5 | 12.2 |
| Sparse, 1 group | 3,000,000 | 244 | 764 | 339 |
| Dense, 1 group | 3,000,000 | 176 | 422 | 339 |
| Sparse, 100 groups | 1,000 | 1.06 | 1.04 | 3.7 |
| Sparse, 100 groups | 3,000,000 | 214 | 384 | 577 |
| Dense, 100 groups | 1,000 | 1.14 | 1.04 | 4.07 |
| Dense, 100 groups | 3,000,000 | 167 | 376 | 612 |

[Exact values, sample ranges and counts](assets/benchmarks/overlap-headline.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).
