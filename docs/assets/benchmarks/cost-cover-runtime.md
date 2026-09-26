![Minimum-cost covering: Rust core. family=chain, order=shuffled, costs=random. Runtime (ms).](assets/benchmarks/cost-cover-runtime.svg)

Shuffled touching chains with random costs. The quadratic reference is measured only at 1,000 rows. Larger cases are absent, not extrapolated. Bands show the observed minimum–maximum range, not confidence intervals.

Source: [covering-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv). [Download plotted values](assets/benchmarks/cost-cover-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Fenwick (production) | 0.0817 | 0.0797 | 0.0841 | 3 |
| 10,000 | Fenwick (production) | 1.013 | 0.9797 | 1.055 | 3 |
| 100,000 | Fenwick (production) | 12.18 | 12.01 | 12.24 | 3 |
| 1,000,000 | Fenwick (production) | 188.1 | 186.5 | 188.9 | 3 |
| 1,000 | Segment tree | 0.108 | 0.1065 | 0.1087 | 3 |
| 10,000 | Segment tree | 1.267 | 1.18 | 1.273 | 3 |
| 100,000 | Segment tree | 14.24 | 13.85 | 14.43 | 3 |
| 1,000,000 | Segment tree | 205.3 | 201.7 | 209.4 | 3 |
| 1,000 | Quadratic reference | 0.0738 | 0.0729 | 0.0743 | 3 |

Measurement: **Runtime (ms)**. Missing cases are omitted, never zero.

</details>
