![Lane assignment: buffer capacity: Rust core. family=moderate128, order=shuffled. Peak live buffer capacity (MiB).](assets/benchmarks/lanes-memory.svg)

The same workload as the runtime chart. Buffer capacity includes algorithm storage and output, excluding caller inputs, allocator overhead, and process RSS.

Source: [assign-lanes-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/assign-lanes-windows.csv). [Download plotted values](assets/benchmarks/lanes-memory.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Heap (production) | 0.0134 | 0.0134 | 0.0134 | 9 |
| 10,000 | Heap (production) | 0.1164 | 0.1164 | 0.1164 | 9 |
| 100,000 | Heap (production) | 1.146 | 1.146 | 1.146 | 9 |
| 1,000,000 | Heap (production) | 11.45 | 11.45 | 11.45 | 9 |
| 1,000 | Two sorts | 0.01909 | 0.01909 | 0.01909 | 9 |
| 10,000 | Two sorts | 0.1908 | 0.1908 | 0.1908 | 9 |
| 100,000 | Two sorts | 1.907 | 1.907 | 1.907 | 9 |
| 1,000,000 | Two sorts | 19.07 | 19.07 | 19.07 | 9 |
| 1,000 | Endpoint events | 0.05008 | 0.05008 | 0.05008 | 9 |
| 10,000 | Endpoint events | 0.4964 | 0.4964 | 0.4964 | 9 |
| 100,000 | Endpoint events | 4.96 | 4.96 | 4.96 | 9 |
| 1,000,000 | Endpoint events | 49.59 | 49.59 | 49.59 | 9 |

Measurement: **Peak live buffer capacity (MiB)**.

</details>
