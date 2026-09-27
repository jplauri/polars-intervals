![Nesting depth: integer and temporal endpoints: Polars end to end. groups=1, family=low8, order=shuffled, candidate=plugin. Collection time (ms).](assets/benchmarks/nesting-temporal.svg)

Shuffled families with maximum depth 7 through an installed release wheel, five samples after warmup. Timing includes lazy optimization/planning, collection and Series retrieval; fixture construction, casts, expression construction and exact validation are excluded.

Source: [nesting-polars-windows.csv](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/nesting-polars-windows.csv). [Download plotted values](assets/benchmarks/nesting-temporal.csv).

<details markdown="1">
<summary>Plotted values and sample counts</summary>

| Input rows | Method | Median | Min | Max | Samples |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1,000 | Int64 | 0.1489 | 0.1282 | 0.1722 | 5 |
| 10,000 | Int64 | 0.5245 | 0.4726 | 0.5465 | 5 |
| 100,000 | Int64 | 5.124 | 5.035 | 5.312 | 5 |
| 1,000,000 | Int64 | 59.74 | 57.95 | 65.18 | 5 |
| 3,000,000 | Int64 | 202.7 | 194.7 | 229.8 | 5 |
| 1,000 | Date | 0.2354 | 0.2115 | 0.3042 | 5 |
| 10,000 | Date | 0.5473 | 0.5262 | 0.7018 | 5 |
| 100,000 | Date | 4.498 | 4.312 | 4.621 | 5 |
| 1,000,000 | Date | 48.58 | 48.16 | 56.87 | 5 |
| 3,000,000 | Date | 170.4 | 164.4 | 179.1 | 5 |
| 1,000 | Datetime(us) | 0.1955 | 0.1706 | 0.2068 | 5 |
| 10,000 | Datetime(us) | 0.5007 | 0.4849 | 0.5634 | 5 |
| 100,000 | Datetime(us) | 4.998 | 4.878 | 5.103 | 5 |
| 1,000,000 | Datetime(us) | 56.78 | 56.37 | 57.85 | 5 |
| 3,000,000 | Datetime(us) | 200.3 | 197.3 | 213.3 | 5 |

Measurement: **Collection time (ms)**.

</details>
