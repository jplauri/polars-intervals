![Maximum-weight selection with capacity: Rust core. family=components, order=shuffled, weights=positive, k=2. Runtime (ms).](assets/benchmarks/capacity-runtime.svg)

Independent components at capacity two. These are the original recorded measurements. The later cleanup run is retained separately and is not pooled into this chart. Bands show the observed minimum–maximum range, not confidence intervals.

Source: [capacity-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv). [Download plotted values](assets/benchmarks/capacity-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Production | 0.2184 | 0.2128 | 0.2374 | 3 |
| 10,000 | Production | 2.214 | 2.21 | 2.285 | 3 |
| 100,000 | Production | 9.688 | 9.62 | 12.49 | 3 |
| 1,000,000 | Production | 128.4 | 121.8 | 142.3 | 3 |
| 1,000 | Serial components | 0.2242 | 0.2124 | 0.2245 | 3 |
| 10,000 | Serial components | 2.268 | 2.224 | 2.271 | 3 |
| 100,000 | Serial components | 24.07 | 24.05 | 24.83 | 3 |
| 1,000,000 | Serial components | 267.8 | 267.7 | 267.8 | 3 |
| 1,000 | Whole-instance flow | 0.2605 | 0.2455 | 0.3323 | 3 |
| 10,000 | Whole-instance flow | 3.241 | 3.21 | 3.579 | 3 |
| 100,000 | Whole-instance flow | 45.79 | 45.54 | 46.53 | 3 |
| 1,000,000 | Whole-instance flow | 501.5 | 496.9 | 504.9 | 3 |
| 1,000 | Generic flow | 0.4643 | 0.4592 | 0.6406 | 3 |
| 10,000 | Generic flow | 7.234 | 6.534 | 7.457 | 3 |
| 100,000 | Generic flow | 125.6 | 124.8 | 127.5 | 3 |
| 1,000,000 | Generic flow | 1969 | 1954 | 2023 | 3 |

Measurement: **Runtime (ms)**. Missing cases are omitted, never zero.

</details>
