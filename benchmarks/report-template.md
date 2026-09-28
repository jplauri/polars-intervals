# <Operation> benchmarks

## Summary

In two or three sentences, link to the operation's API/usage documentation,
state the main measured production result, and explain the principal tradeoff.

## Results

Polars collection · <thread count> · median of <N> samples · [run metadata](<link>)

Include a generated compact table: representative workload/size, absolute
production time with units, and descriptive baseline names. Mark production
explicitly. Include a typical case, a meaningful alternative, and important
losses. Explain any per-case baseline selection rule and name the selected method.

Interpret the table briefly without reciting its numbers. Use at most one plot,
only for a useful scaling difference, crossover, or runtime/memory tradeoff.
Where measured, put Polars collections before a separately labelled Rust table;
otherwise say that coverage is Rust-only. Give each set its own scope/metadata
line. Keep material timing or instrumentation exceptions next to the table.

## Coverage and limitations

Briefly describe measured dimensions and missing coverage. State the validation
actually used and link to details; do not call every check an independent oracle.
Keep important losses, bounded baselines/skips, historical adapter differences,
and material memory tradeoffs visible. Name the applicable memory metric and link
to [shared definitions](../docs/benchmarking.md#memory-metrics).

## Reproduce and data

<details markdown="1">
<summary>Reproduce this operation</summary>

Follow the [shared setup instructions](../docs/benchmarking.md#setup), then run:

```sh
<operation-specific command, writing a new result file>
```

</details>

Link to raw samples (including repeat runs), run metadata/source hashes, and
supporting design/correctness notes. Keep derivations, inventories, rejected
approaches, and chronology in those notes, not in the main report.

<!-- Author guidance: keep these four headings in order. Aim for roughly 400–600
visible words excluding tables/collapsed material, without padding or dropping
qualifications. Hardware has one home in docs/benchmarks.md; common methodology,
setup and publishing live in docs/benchmarking.md. Use ~3 significant figures;
exact generated values remain in CSV. Adjust relative links when copying to docs/. -->
