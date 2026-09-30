# <Operation> benchmarks

[All benchmarks](../docs/benchmarks.md)

## Summary

Write one short paragraph, usually 50–80 words. Start with the linked method
name and explain its purpose in one sentence. Give a measured input size,
runtime, and comparison with the fastest native Polars method tested on the
same workload. Use Polars' default thread pool for headline comparisons. If only
restricted-thread measurements exist, put that condition in the comparison
sentence itself, such as "with Polars restricted to one thread". Never label a
fixed thread count as the default unless the run used the default setting.

State an exception readers can act on, such as input order, group size, weights
or thread settings. Keep comparisons with private candidate algorithms in
details. Identify synthetic examples as such. Keep API options in the API
reference and implementation history in details.

If complete Polars timings are missing, say "Full Polars query timings have not
been measured." Prefer measuring the public operation before publishing.
For an optimizer without a built-in Polars solver, use "Polars has no built-in
solver for this optimization problem." For an unmeasured native expression
comparison, use "A native Polars comparison has not been measured."
For heuristics, pair runtime with the quality of the result. Distinguish
observed improvements from guarantees.

## Results

**Full Polars query time · milliseconds**

Use one small generated table with this shape:

| Workload | Rows | polars-intervals (ms) | Native Polars (ms) | Compared with native |
| --- | ---: | ---: | ---: | --- |
| <Plain-language input description> | <Size> | <Time> | <Time> | <Ratio>× faster |

Add one visible line defining the comparison in public Polars terms, such as
"Native Polars: the fastest tested query using joins or `search_sorted`."
Native Polars means the fastest equivalent query tested for that row in the same run.
Record which method won in Benchmark details. Include small and large inputs
where measured, plus important cases where the advantage disappears or reverses.
Use about three significant figures. Missing measurements stay missing.
Describe inconclusive differences as "about the same" and losses as "× slower".

When no native comparison exists, show workload, rows and the package runtime.
Keep useful input variants or public modes in separate, clearly named columns.
An operation with a different purpose is not a native Polars baseline. Label
algorithm-only measurements as package algorithm time.

Follow the table with one or two sentences explaining the takeaway. Discuss
effects readers recognize, such as input order, overlap, grouping or selection
limits. Keep meaningful variability visible when it could change the conclusion.
Use a plot only when it makes a scaling trend, crossover or tradeoff clearer.

Add brief notes only when they change how readers should interpret the result.
These can cover missing measurements, restricted comparisons, meaningful memory
costs or an older measured build. Explain them in plain language. An older-build
note must name the measured version or date and say which timed path changed,
or that no relevant path changed. Do not repeat a vague historical disclaimer.
Keep source hashes and exact revisions in details. Put exact thread settings in
details, except for the summary's restricted-thread condition.
For algorithm-only results, replace the timing label with "Underlying algorithm
time" and keep them separate from full Polars measurements.

<details markdown="1">
<summary>Benchmark details</summary>

Link to the [measurement guide](../docs/benchmarking.md) and shared hardware.
Preserve timing boundaries, sample counts, warmups, thread settings, seeds,
versions, source hashes, raw samples, exact table downloads and repeat runs.
Keep runs and timing scopes separate. Explain case selection and identify the
native method used for each row. Define speedup as native median / package median.
For a loss, describe "× slower" using package median / native median.

Keep detailed workload coverage, correctness evidence, memory definitions,
algorithm comparisons and implementation history here or in supporting notes.
Distinguish independent checks from agreement between implementations. Preserve
unfavorable results and measurement gaps. Label proposed explanations as such.

Include the operation's reproduction command and link to the shared setup.
Add only the details needed for this operation.

</details>

<!-- Acceptance test: after the summary and first table, a newcomer knows the
measured runtime, how it compares with the fastest tested native Polars method,
and the main exception. Reproduction and implementation details stay optional.
Preserve existing page URLs and anchors when updating reports. Keep headings
inside details out of the table of contents, preserving their IDs explicitly. -->
