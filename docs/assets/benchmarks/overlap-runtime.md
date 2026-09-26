![Overlap counting: Polars end to end. scenario=sparse, groups=1, order=shuffled, dtype=int64. Collection time (ms).](assets/benchmarks/overlap-runtime.svg)

Sparse, ungrouped Int64 input from the original 24-thread Windows run. The plot compares the plugin with four native counting formulations.

Source: [sweep-windows.json](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/sweep-windows.json). [Download plotted values](assets/benchmarks/overlap-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Plugin | 0.2778 | 0.2335 | 0.4515 | 9 |
| 100,000 | Plugin | 6.154 | 5.701 | 6.283 | 9 |
| 1,000,000 | Plugin | 71.58 | 68.25 | 75.15 | 9 |
| 3,000,000 | Plugin | 243.7 | 236.9 | 250.2 | 9 |
| 1,000 | Search sorted | 0.4909 | 0.3958 | 0.6178 | 9 |
| 100,000 | Search sorted | 10.54 | 10.27 | 11.2 | 9 |
| 1,000,000 | Search sorted | 164.4 | 149.5 | 203.3 | 9 |
| 3,000,000 | Search sorted | 764.1 | 736 | 840.7 | 9 |
| 1,000 | Parallel search sorted | 0.5372 | 0.4676 | 0.6535 | 9 |
| 100,000 | Parallel search sorted | 10.71 | 10.44 | 10.9 | 9 |
| 1,000,000 | Parallel search sorted | 160.6 | 146.2 | 204.2 | 9 |
| 3,000,000 | Parallel search sorted | 775.8 | 745.8 | 832.1 | 9 |
| 1,000 | As-of joins | 2.377 | 2.218 | 2.841 | 9 |
| 100,000 | As-of joins | 12.24 | 11.52 | 20.39 | 9 |
| 1,000,000 | As-of joins | 106.4 | 98.13 | 151.7 | 9 |
| 3,000,000 | As-of joins | 339.3 | 336.1 | 395 | 9 |
| 1,000 | Endpoint sweep | 2.595 | 2.452 | 3.12 | 9 |
| 100,000 | Endpoint sweep | 16.65 | 15.15 | 18.87 | 9 |
| 1,000,000 | Endpoint sweep | 158.7 | 153.4 | 175.7 | 9 |
| 3,000,000 | Endpoint sweep | 529.3 | 511.7 | 601.2 | 9 |

Measurement: **Collection time (ms)**.

</details>
