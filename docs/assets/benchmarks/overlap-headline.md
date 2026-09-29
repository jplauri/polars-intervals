**Polars query time (ms) · medians**

| Workload | Input rows | overlap_count | Polars search expressions | Polars as-of joins |
| --- | ---: | ---: | ---: | ---: |
| Few overlaps, 1 group | 100,000 | 6.15 | 10.5 | 12.2 |
| Few overlaps, 1 group | 3,000,000 | 244 | 764 | 339 |
| Many overlaps, 1 group | 3,000,000 | 176 | 422 | 339 |
| Few overlaps, 100 groups | 1,000 | 1.06 | 1.04 | 3.7 |
| Few overlaps, 100 groups | 3,000,000 | 214 | 384 | 577 |
| Many overlaps, 100 groups | 1,000 | 1.14 | 1.04 | 4.07 |
| Many overlaps, 100 groups | 3,000,000 | 167 | 376 | 612 |

[Exact values, sample ranges and counts](assets/benchmarks/overlap-headline.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json).
