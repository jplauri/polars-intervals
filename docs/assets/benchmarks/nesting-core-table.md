**Algorithm time (ms) · medians**

| Workload | Input rows | Package design (frontier) | Packed Fenwick | Indirect Fenwick |
| --- | ---: | ---: | ---: | ---: |
| Fully nested, shuffled | 3,000,000 | 199 | 1,220 | 1,450 |
| Dense random | 3,000,000 | 250 | 376 | 731 |
| All identical | 3,000,000 | 36.1 | 48.2 | 32.5 |

[Exact values, sample ranges and counts](assets/benchmarks/nesting-core-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv).
