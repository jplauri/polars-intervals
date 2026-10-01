**Times in milliseconds · medians**

| Workload | Queries n = sources m | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | ---: |
| Genomic reads and overlapping windows | 1,000 | 1.02 | 9.05 | 8.9× faster |
| Genomic reads and overlapping windows | 1,000,000 | 467 | 3,810 | 8.2× faster |
| Genomic reads, seed 41 repeat | 1,000,000 | 474 | 3,990 | 8.4× faster |
| Disjoint sources | 1,000 | 1.41 | 11.1 | 7.9× faster |
| Disjoint sources | 1,000,000 | 353 | 3,920 | 11× faster |
| Nested windows | 1,000,000 | 189 | 4,110 | 22× faster |
| One source-union run with many matches | 1,000,000 | 99.5 | 1,980 | 20× faster |
| Many tiny timestamp groups | 1,000 | 3.21 | 17.9 | 5.6× faster |

[Exact values, sample ranges and counts](assets/benchmarks/coverage-stats-summary.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/coverage-stats-polars-million.csv).
