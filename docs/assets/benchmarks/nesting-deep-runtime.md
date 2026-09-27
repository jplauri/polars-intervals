![Nesting depth: one deep chain: Rust core. dtype=i64, scenario=chain, groups=1. Runtime (ms).](assets/benchmarks/nesting-deep-runtime.svg)

A shuffled strict chain with maximum depth n−1. Directly appending an eligible deepest-chain endpoint avoids a binary search for each new level.

Source: [nesting-kernels-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv). [Download plotted values](assets/benchmarks/nesting-deep-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Packed Fenwick | 0.0669 | 0.0612 | 0.0866 | 5 |
| 10,000 | Packed Fenwick | 0.9628 | 0.7195 | 1.002 | 5 |
| 100,000 | Packed Fenwick | 11.29 | 11.02 | 12.32 | 5 |
| 1,000,000 | Packed Fenwick | 171.5 | 165.9 | 195.1 | 5 |
| 3,000,000 | Packed Fenwick | 1221 | 1210 | 1267 | 5 |
| 1,000 | Indirect Fenwick | 0.0676 | 0.0566 | 0.0814 | 5 |
| 10,000 | Indirect Fenwick | 1.191 | 0.8434 | 1.229 | 5 |
| 100,000 | Indirect Fenwick | 11.92 | 11.81 | 12.12 | 5 |
| 1,000,000 | Indirect Fenwick | 216.7 | 211.1 | 240.1 | 5 |
| 3,000,000 | Indirect Fenwick | 1448 | 1410 | 1526 | 5 |
| 1,000 | Segment tree | 0.1125 | 0.1026 | 0.144 | 5 |
| 10,000 | Segment tree | 1.632 | 1.323 | 1.835 | 5 |
| 100,000 | Segment tree | 17.85 | 17.68 | 18.3 | 5 |
| 1,000,000 | Segment tree | 276 | 264.1 | 301.6 | 5 |
| 3,000,000 | Segment tree | 1455 | 1376 | 1521 | 5 |
| 1,000 | Frontier | 0.059 | 0.0563 | 0.0713 | 5 |
| 10,000 | Frontier | 0.5891 | 0.5772 | 0.7939 | 5 |
| 100,000 | Frontier | 7.871 | 7.819 | 7.972 | 5 |
| 1,000,000 | Frontier | 95.21 | 94.82 | 96.8 | 5 |
| 3,000,000 | Frontier | 335.1 | 329.6 | 338.4 | 5 |
| 1,000 | Frontier + append | 0.0394 | 0.0381 | 0.0631 | 5 |
| 10,000 | Frontier + append | 0.3976 | 0.2917 | 0.4049 | 5 |
| 100,000 | Frontier + append | 4.131 | 4.092 | 4.265 | 5 |
| 1,000,000 | Frontier + append | 55.45 | 55.03 | 56.93 | 5 |
| 3,000,000 | Frontier + append | 199.5 | 198.7 | 220.9 | 5 |

Measurement: **Runtime (ms)**.

</details>
