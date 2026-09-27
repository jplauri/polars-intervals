![Nesting depth: shallow families: Rust core. dtype=i64, scenario=depth_4, groups=1. Runtime (ms).](assets/benchmarks/nesting-shallow-runtime.svg)

Shuffled disjoint families of five nested geometries (maximum depth 4). Five release samples per size on one Windows desktop; candidate totals include phase clocks.

Source: [nesting-kernels-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv). [Download plotted values](assets/benchmarks/nesting-shallow-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Packed Fenwick | 0.0657 | 0.0535 | 0.194 | 5 |
| 10,000 | Packed Fenwick | 0.7983 | 0.7273 | 0.912 | 5 |
| 100,000 | Packed Fenwick | 10.8 | 10.7 | 11.13 | 5 |
| 1,000,000 | Packed Fenwick | 169 | 167.7 | 177.3 | 5 |
| 3,000,000 | Packed Fenwick | 1114 | 1088 | 1660 | 5 |
| 1,000 | Indirect Fenwick | 0.0768 | 0.0659 | 0.0834 | 5 |
| 10,000 | Indirect Fenwick | 0.8093 | 0.7957 | 1.067 | 5 |
| 100,000 | Indirect Fenwick | 11.63 | 11.52 | 11.65 | 5 |
| 1,000,000 | Indirect Fenwick | 218.5 | 210.9 | 230.8 | 5 |
| 3,000,000 | Indirect Fenwick | 1438 | 1428 | 1458 | 5 |
| 1,000 | Segment tree | 0.1199 | 0.0981 | 0.1291 | 5 |
| 10,000 | Segment tree | 1.416 | 1.29 | 1.771 | 5 |
| 100,000 | Segment tree | 17.74 | 17.59 | 17.82 | 5 |
| 1,000,000 | Segment tree | 248 | 241.8 | 292.6 | 5 |
| 3,000,000 | Segment tree | 1395 | 1302 | 1488 | 5 |
| 1,000 | Frontier + append | 0.0344 | 0.0272 | 0.037 | 5 |
| 10,000 | Frontier + append | 0.3908 | 0.3629 | 0.4389 | 5 |
| 100,000 | Frontier + append | 4.607 | 4.57 | 4.704 | 5 |
| 1,000,000 | Frontier + append | 56.15 | 56.12 | 58.62 | 5 |
| 3,000,000 | Frontier + append | 196.2 | 190.2 | 204.2 | 5 |

Measurement: **Runtime (ms)**.

</details>
