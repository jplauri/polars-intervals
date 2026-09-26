![Minimum-cardinality covering: Rust core. family=chain, order=shuffled, costs=random. Runtime (ms).](assets/benchmarks/cover-runtime.svg)

Shuffled touching chains from the shared covering harness. The cost distribution identifies the fixture. Minimum-cardinality covering does not optimize costs. Bands show the observed minimum–maximum range, not confidence intervals.

Source: [covering-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/covering-windows.csv). [Download plotted values](assets/benchmarks/cover-runtime.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Packed greedy (production) | 0.0199 | 0.0194 | 0.0212 | 3 |
| 10,000 | Packed greedy (production) | 0.3433 | 0.294 | 0.4092 | 3 |
| 100,000 | Packed greedy (production) | 4.16 | 4.146 | 4.257 | 3 |
| 1,000,000 | Packed greedy (production) | 50.07 | 49.94 | 50.53 | 3 |
| 1,000 | Indirect greedy | 0.0296 | 0.0285 | 0.0318 | 3 |
| 10,000 | Indirect greedy | 0.3867 | 0.3859 | 0.3942 | 3 |
| 100,000 | Indirect greedy | 5.02 | 4.935 | 5.053 | 3 |
| 1,000,000 | Indirect greedy | 84.18 | 82.07 | 91.15 | 3 |
| 1,000 | Heap | 0.024 | 0.0235 | 0.0287 | 3 |
| 10,000 | Heap | 0.3893 | 0.3875 | 0.419 | 3 |
| 100,000 | Heap | 4.594 | 4.502 | 5.343 | 3 |
| 1,000,000 | Heap | 54.84 | 54.52 | 58.41 | 3 |
| 1,000 | Packed with sortedness check | 0.0195 | 0.0191 | 0.0215 | 3 |
| 10,000 | Packed with sortedness check | 0.2888 | 0.2853 | 0.3511 | 3 |
| 100,000 | Packed with sortedness check | 4.162 | 4.12 | 4.446 | 3 |
| 1,000,000 | Packed with sortedness check | 53.36 | 50.33 | 55.06 | 3 |

Measurement: **Runtime (ms)**. Missing cases are omitted, never zero.

</details>
