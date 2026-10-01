**Times in milliseconds · medians**

| Workload | Total input rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Subtract empty-only grouped rows | 100,000 | 4.67 | 6.6 | 1.4× faster |
| Subtract empty-only grouped rows | 1,000,000 | 32.6 | 15.4 | 2.1× slower |
| Intersect empty-only grouped rows | 100,000 | 4.52 | 6.99 | 1.5× faster |
| Intersect empty-only grouped rows | 1,000,000 | 33.9 | 16.2 | 2.1× slower |

[Exact values, sample ranges and counts](assets/benchmarks/set-geometry-empty-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/set-geometry-polars-empty-repeat-20261001.csv).
