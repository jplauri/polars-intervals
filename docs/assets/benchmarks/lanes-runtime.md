![Lane assignment: Rust core. family=moderate128, order=shuffled. Runtime (ms).](assets/benchmarks/lanes-runtime.svg)

Shuffled input with concurrency 128, first run. This workload illustrates a case where the production choice loses at large sizes. The full report covers the wider tradeoff. Bands show the observed minimum–maximum range, not confidence intervals.

Source: [assign-lanes-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv). [Download plotted values](assets/benchmarks/lanes-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Heap (production) | 0.0388 | 0.0362 | 0.0433 | 9 |
| 10,000 | Heap (production) | 0.562 | 0.549 | 0.5644 | 9 |
| 100,000 | Heap (production) | 5.741 | 5.609 | 5.884 | 9 |
| 1,000,000 | Heap (production) | 153.1 | 148.8 | 162.1 | 9 |
| 1,000 | Two sorts | 0.0469 | 0.0412 | 0.0534 | 9 |
| 10,000 | Two sorts | 0.634 | 0.6072 | 0.6406 | 9 |
| 100,000 | Two sorts | 7.455 | 7.176 | 7.799 | 9 |
| 1,000,000 | Two sorts | 140.1 | 139.5 | 143.3 | 9 |
| 1,000 | Endpoint events | 0.0592 | 0.0566 | 0.064 | 9 |
| 10,000 | Endpoint events | 0.6751 | 0.6662 | 0.8255 | 9 |
| 100,000 | Endpoint events | 7.661 | 7.552 | 7.922 | 9 |
| 1,000,000 | Endpoint events | 100.1 | 98.73 | 101.5 | 9 |

Measurement: **Runtime (ms)**. Missing cases are omitted, never zero.

</details>
