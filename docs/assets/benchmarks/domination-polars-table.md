**Polars collection time (ms) · medians**

| Workload | Input rows | Implicit units | Explicit ones | All zero | Mixed costs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shuffled Int64 path | 8 | 0.0639 | 0.0729 | 0.0727 | 0.0702 |
| Shuffled Int64 path | 1,000 | 0.115 | 0.123 | 0.124 | 0.179 |
| Shuffled Int64 path | 100,000 | 7.97 | 8.77 | 8.72 | 18 |
| Full clique, shuffled Date | 100,000 | 1.41 | 2.09 | 2.1 | 6.67 |
| Path, shuffled UTC ns, streaming chunks | 100,000 | 9.01 | 9.88 | 10.4 | 19.2 |
| 32 interleaved groups, shuffled Int64 path | 1,000 | 0.385 | 0.481 | 0.473 | 0.502 |
| 32 interleaved groups, shuffled Int64 path | 100,000 | 7.94 | 8.61 | 8.84 | 14.3 |

[Exact values, sample ranges and counts](assets/benchmarks/domination-polars-table.csv) · [Source samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/domination-polars-windows-20260929.timings.csv).
