**Polars collection time (ms) · medians**

| Workload | Input rows | Implicit units | Explicit ones | Positive weights | Mixed signs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Concurrency 8, shuffled Int64 | 1,000 | 0.0803 | 0.0985 | 0.0972 | 0.0788 |
| Concurrency 8, shuffled Int64 | 100,000 | 3.87 | 7.38 | 7.45 | 3.26 |
| Concurrency 8, shuffled Int64 | 1,000,000 | 43.1 | 148 | 140 | 45.5 |
| Full clique, shuffled Date | 1,000,000 | 2.06 | 11 | 10.6 | 12.6 |
| Concurrency 8, shuffled UTC ns, multichunk streaming | 1,000,000 | 48.2 | 129 | 126 | 48.6 |
| 32 interleaved groups, shuffled Int64, multichunk | 1,000 | 0.414 | 0.409 | 0.437 | 0.433 |
| 32 interleaved groups, shuffled Int64, multichunk | 1,000,000 | 49.6 | 82.9 | 83.1 | 44.7 |

[Exact values, sample ranges and counts](assets/benchmarks/clique-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.timings.csv).
