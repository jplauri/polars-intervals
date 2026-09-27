![Nesting depth: deep-chain memory: Rust core. dtype=i64, scenario=chain, groups=1. Peak requested heap (MiB).](assets/benchmarks/nesting-memory.svg)

One separate allocation measurement per case, repeated in five timing rows (the table's sample count). Includes output; excludes inputs, allocator overhead and process RSS. Frontier capacity grows geometrically with depth and can exceed Fenwick memory on deep chains.

Source: [nesting-kernels-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-kernels-windows.csv). [Download plotted values](assets/benchmarks/nesting-memory.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Packed Fenwick | 0.03815 | 0.03815 | 0.03815 | 5 |
| 10,000 | Packed Fenwick | 0.3815 | 0.3815 | 0.3815 | 5 |
| 100,000 | Packed Fenwick | 3.815 | 3.815 | 3.815 | 5 |
| 1,000,000 | Packed Fenwick | 38.15 | 38.15 | 38.15 | 5 |
| 3,000,000 | Packed Fenwick | 114.4 | 114.4 | 114.4 | 5 |
| 1,000 | Indirect Fenwick | 0.0229 | 0.0229 | 0.0229 | 5 |
| 10,000 | Indirect Fenwick | 0.2289 | 0.2289 | 0.2289 | 5 |
| 100,000 | Indirect Fenwick | 2.289 | 2.289 | 2.289 | 5 |
| 1,000,000 | Indirect Fenwick | 22.89 | 22.89 | 22.89 | 5 |
| 3,000,000 | Indirect Fenwick | 68.66 | 68.66 | 68.66 | 5 |
| 1,000 | Segment tree | 0.04578 | 0.04578 | 0.04578 | 5 |
| 10,000 | Segment tree | 0.4578 | 0.4578 | 0.4578 | 5 |
| 100,000 | Segment tree | 4.578 | 4.578 | 4.578 | 5 |
| 1,000,000 | Segment tree | 45.78 | 45.78 | 45.78 | 5 |
| 3,000,000 | Segment tree | 137.3 | 137.3 | 137.3 | 5 |
| 1,000 | Frontier + append | 0.03833 | 0.03833 | 0.03833 | 5 |
| 10,000 | Frontier + append | 0.4302 | 0.4302 | 0.4302 | 5 |
| 100,000 | Frontier + append | 4.052 | 4.052 | 4.052 | 5 |
| 1,000,000 | Frontier + append | 38.52 | 38.52 | 38.52 | 5 |
| 3,000,000 | Frontier + append | 123.6 | 123.6 | 123.6 | 5 |

Measurement: **Peak requested heap (MiB)**.

</details>
