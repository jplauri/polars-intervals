# <Operation> benchmarks

[All benchmarks](../docs/benchmarks.md) · [Measurement rules](../docs/benchmarking.md#measurement-rules)

## Summary

Write one short paragraph, usually 50–80 words. Start with the linked method
name and explain its purpose in one sentence. Give a measured input size,
runtime, and comparison with the fastest native Polars method tested on the
same workload. Headline runtimes and comparisons use Polars' default thread
pool or all logical processors. If the headline workload was measured only with
fewer threads, put that condition in the summary sentence itself, such as "with
Polars restricted to one thread". If other workloads were measured with all
logical processors, add one visible sentence with those results. Never label a
fixed thread count as the default unless the run used the default setting.

State an exception readers can act on, such as input order, group size, weights
or thread settings. Keep comparisons with private candidate algorithms in
details. Identify synthetic examples as such. Keep API options in the API
reference and implementation history in details. Use "function" in visible text.

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
Use about three significant figures for times and two for ratios. Quote ratios
in the summary and overview as the table shows them. Missing measurements stay
missing.
Describe inconclusive differences as "about the same" and losses as "× slower".

Use one ratio convention: **speedup = baseline median / package median**.
A value of 2 means the package is 2× faster; a value of 0.5 means it is 2× slower.
Do not mix speedup and relative-runtime ratios in new report tables.

When no native comparison exists, show workload, rows and the package runtime.
Keep useful input variants or public modes in separate, clearly named columns.
An operation with a different purpose is not a native Polars baseline. Label
algorithm-only measurements as package algorithm time.

Follow the table with one or two sentences explaining the takeaway. Discuss
effects readers recognize, such as input order, overlap, grouping or selection
limits. Keep meaningful variability visible when it could change the conclusion.
Use a plot only when it makes a scaling trend, crossover or tradeoff clearer.

Add brief notes only when they change how readers should interpret the result.
These can cover missing measurements, restricted comparisons or meaningful
memory costs. Explain them in plain language. Do not mention measurement dates
or older builds. Keep source hashes and exact revisions in details. Put exact
thread settings in details, except for the summary's thread condition.
For algorithm-only results, replace the timing label with "Underlying algorithm
time" and keep them separate from full Polars measurements.

<details markdown="1">
<summary>Benchmark details</summary>

Use the following five bold subsection labels in this order. Keep each short.
Link long derivations and experiment narratives from supporting notes instead
of filling this block. Keep these labels out of the table of contents.

**What was compared**

Identify the public function, equivalent native queries and selected workloads.
Name the fastest native method for each headline row. Explain case selection
and retain useful exceptions. Include the headline table's exact downloads.
Additional results belong here only when they help readers interpret the first
table. Keep separate runs and scopes separate.

**Settings**

Give timing boundaries, warmups, samples, threads, seeds and software versions.
Briefly explain correctness evidence, distinguishing independent checks from
agreement between candidates. Link the shared hardware and measurement guide
instead of repeating them. Preserve settings and source/build hashes in linked
metadata.

**Limitations**

State missing input types, workloads, baselines and execution modes. Name any
memory metric and exclusions. Keep meaningful variability and unfavorable
results. Label proposed explanations as such.

**Reproduce**

Give the operation command and link the shared setup. Include raw samples,
metadata, measured source archives and supplemental table downloads here.

**History**

Keep older builds, private algorithm comparisons and repeat-run history here.
Identify measured builds by revision, not date. Use supporting notes for lengthy
development narratives. Preserve original measurement identities and archived
table conventions without relabeling data. Explain how timed paths changed,
without inferring an unmeasured speedup.

</details>

<!-- Acceptance test: after the summary and first table, a newcomer knows the
measured runtime, how it compares with the fastest tested native Polars method,
and the main exception. Reproduction and implementation details stay optional.
Preserve existing page URLs and anchors when updating reports. Keep headings
inside details out of the table of contents, preserving their IDs explicitly.
Use the same operation name in the page title, sidebar and overview. -->
