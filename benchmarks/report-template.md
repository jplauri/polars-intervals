# <Operation> benchmarks

[All benchmarks](../docs/benchmarks.md) · [Measurement guide](../docs/benchmarking.md)

## Summary

Start with the exact method name in code formatting, linked to its API reference
(for example, [`minimum_cover`](../docs/api.md#polars_intervals.minimum_cover)).
Explain what it does before discussing performance. Write for a reader who knows
their interval data but has never seen the implementation or benchmark setup.

Give a concrete input size and measured runtime early, with the conditions needed
to interpret them. Identify selected synthetic examples as such. State whether
the time measures a complete Polars operation or only its underlying algorithm.
Explain the main benefit and limitation in a few direct sentences. For heuristic
methods, describe result quality in everyday terms and distinguish observed
improvements from guarantees. For exact methods, state what is guaranteed.

## Results

Complete Polars query / Rust algorithm only · <thread count> · median of <N>
samples · [measurement settings](<link>)

Include a generated compact table with clear workload descriptions, input sizes,
absolute times with units, and recognizable method names. Identify the package's
method and explain alternatives. Define necessary shorthand beside the table.
Include representative measured cases and important slow cases. Explain any
per-case selection of comparison methods. Keep exact data and sample ranges
accessible through the generated downloads.

Explain the main takeaway without reciting every cell. Describe effects readers
recognize, such as input order, overlap, grouping, capacity or selection limits,
before internal causes. For heuristics, connect runtime to the quality gained
on the same inputs. Use at most one plot when it makes a useful comparison clearer.

Put measured Polars operations before algorithm-only comparisons. Keep detailed
algorithm comparisons in an expandable block within Results or in supporting
notes. Give every table a timing scope and measurement-settings link. Identify
benchmark implementations that represent the package's design rather than direct
calls to its current public function. Keep qualifications needed to understand
the numbers next to them.

## Coverage and limitations

Explain how varied the test data was, how results were checked, and where a user's
experience might differ. Distinguish exhaustive small-case checks from agreement
between implementations on large inputs. For heuristics, distinguish improvement
from reaching a known best result. Do not imply that best results are known for
every dataset or that the test mix represents users' workloads.

Keep consequential limits visible: missing Polars timings, unmeasured input types,
restricted comparisons, meaningful slow cases, memory costs and older measured
builds. Explain memory figures as algorithm storage or whole-process memory and
link to [precise definitions](../docs/benchmarking.md#memory-metrics). Put internal
instrumentation, historical experiments and detailed validation in supporting notes.

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

<!-- Author guidance: keep these four headings in order and both navigation links
at the top. Use direct sentences, avoid semicolons, and introduce concepts before
using them. Avoid unexplained notation, benchmark jargon and promotional claims.
Keep prose short without padding or dropping material qualifications. After the
summary and first table, a newcomer should understand the performance and main
limitation. Hardware has one home in docs/benchmarks.md. Shared methodology, setup
and publishing live in docs/benchmarking.md. Use about three significant figures.
Exact generated values remain in CSV. Adjust relative links when copying to docs/. -->
