![Maximum-weight non-overlapping selection: Rust core. family=moderate128, order=shuffled, weights=positive. Runtime (ms).](assets/benchmarks/weighted-runtime.svg)

Positive weights and moderate overlap, first run. Random-length workloads can favor other candidates. This plot does not aggregate those workloads. Bands show the observed minimum–maximum range, not confidence intervals.

Source: [weighted-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/weighted-windows.csv). [Download plotted values](assets/benchmarks/weighted-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | A: binary search | 0.0435 | 0.0407 | 0.0477 | 5 |
| 10,000 | A: binary search | 0.4538 | 0.4477 | 0.5589 | 5 |
| 100,000 | A: binary search | 9.332 | 7.085 | 10.65 | 5 |
| 1,000,000 | A: binary search | 173.2 | 171.4 | 216.9 | 5 |
| 1,000 | B: two orders (production) | 0.0356 | 0.0299 | 0.0381 | 5 |
| 10,000 | B: two orders (production) | 0.2987 | 0.2966 | 0.4191 | 5 |
| 100,000 | B: two orders (production) | 5.089 | 4.881 | 6.807 | 5 |
| 1,000,000 | B: two orders (production) | 100.8 | 83.75 | 116.9 | 5 |
| 1,000 | C: endpoint events | 0.0628 | 0.0452 | 0.0702 | 5 |
| 10,000 | C: endpoint events | 0.7291 | 0.6408 | 0.7389 | 5 |
| 100,000 | C: endpoint events | 12.97 | 9.459 | 13.37 | 5 |
| 1,000,000 | C: endpoint events | 169.7 | 167.4 | 211.7 | 5 |

Measurement: **Runtime (ms)**. Missing cases are omitted, never zero.

</details>
