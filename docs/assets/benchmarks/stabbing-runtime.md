![Minimum stabbing points: Rust core. dtype=i64, family=disjoint, order=shuffled. Runtime (ms).](assets/benchmarks/stabbing-runtime.svg)

Shuffled disjoint Int64 intervals. Candidate timings include phase-clock overhead, while the production timing excludes it.

Source: [stabbing-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/stabbing-windows.csv). [Download plotted values](assets/benchmarks/stabbing-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | A: packed | 0.0124 | 0.0123 | 0.0127 | 3 |
| 10,000 | A: packed | 0.3116 | 0.2869 | 0.364 | 3 |
| 100,000 | A: packed | 2.85 | 2.724 | 2.921 | 3 |
| 1,000,000 | A: packed | 32.59 | 30.95 | 36.77 | 3 |
| 3,000,000 | A: packed | 108.8 | 105.8 | 110 | 3 |
| 1,000 | B: indirect | 0.0173 | 0.0157 | 0.0203 | 3 |
| 10,000 | B: indirect | 0.3572 | 0.3458 | 0.4007 | 3 |
| 100,000 | B: indirect | 3.688 | 3.486 | 4.921 | 3 |
| 1,000,000 | B: indirect | 55.16 | 54.27 | 57.09 | 3 |
| 3,000,000 | B: indirect | 283.3 | 273.6 | 292.3 | 3 |
| 1,000 | C: detect + packed | 0.0126 | 0.0124 | 0.0128 | 3 |
| 10,000 | C: detect + packed | 0.2814 | 0.2768 | 0.2868 | 3 |
| 100,000 | C: detect + packed | 2.922 | 2.726 | 3.597 | 3 |
| 1,000,000 | C: detect + packed | 31.68 | 31.02 | 34.83 | 3 |
| 3,000,000 | C: detect + packed | 106.8 | 105.6 | 115 | 3 |
| 1,000 | Production (no phase clocks) | 0.0124 | 0.0124 | 0.0128 | 3 |
| 10,000 | Production (no phase clocks) | 0.2915 | 0.2819 | 0.2999 | 3 |
| 100,000 | Production (no phase clocks) | 2.759 | 2.75 | 3.717 | 3 |
| 1,000,000 | Production (no phase clocks) | 30.76 | 30.75 | 31.86 | 3 |
| 3,000,000 | Production (no phase clocks) | 112.7 | 110.1 | 115.5 | 3 |

Measurement: **Runtime (ms)**.

</details>
