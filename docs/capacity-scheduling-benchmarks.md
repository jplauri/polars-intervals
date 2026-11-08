# Capacity scheduling benchmarks

[All benchmarks](benchmarks.md) · [Measurement rules](benchmarking.md#measurement-rules)

## Summary

[`max_weight_with_capacity`](api.md#polars_intervals.max_weight_with_capacity)
selects intervals with the highest total weight while limiting how many may
overlap at once. In synthetic examples with small, independent overlap groups,
full Polars queries took **under 170 ms for one million intervals** and
returned an exact optimum. Polars has no built-in solver for this optimization
problem. Large, interconnected groups can take much longer.

## Results

**Full Polars query time · milliseconds**

--8<-- "docs/assets/benchmarks/capacity-summary.md:3:-3"

Each overlap group has 32 intervals. A limit of 31 still requires choosing one
to leave out, while a limit of 32 allows all intervals in these examples and
makes the task easier.

<details markdown="1">
<summary>Benchmark details</summary>

<span id="full-polars-measurements"></span>

**What was compared**

The headline table uses integer endpoints at one million rows and shows overlap
limits from selecting one interval per group through selecting all 32.

--8<-- "docs/assets/benchmarks/capacity-summary.md:-2:"

Each set contains 32 intervals that overlap one another, with no overlap between
sets. Capacity is the maximum number that may be selected at the same time.
The columns compare integer, Date and microsecond Datetime endpoints.

--8<-- "docs/assets/benchmarks/capacity-polars-table.md"

Runtime does not simply increase with capacity. A capacity of 31 still requires
choosing which interval to leave out. At 32, every interval in these positive-
weight examples can be selected, making the task much easier.

**Settings**

See the [measurement guide](benchmarking.md) and [shared hardware](benchmarks.md#hardware).

**Complete Polars queries · solver uses up to 8 workers · median of 3 samples ·
Polars' default 24-thread pool · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow.metadata.json)**

Small cases are checked by trying all subsets. Larger results are checked for
capacity violations and compared with an independent optimization implementation.
The Polars examples also have independently calculable best weights.

Query times include validation and output construction, but exclude generating
and converting input data.

<span id="coverage-and-limitations"></span>

**Limitations**

Algorithm tests cover empty inputs through one million rows, varied overlap,
sorted and shuffled order, seven weight distributions and different capacity
limits. Polars tests cover integer, Date and Datetime endpoints, including
timezone-aware nanosecond timestamps. No equivalent Polars-only optimizer was
benchmarked.

The algorithm-only datasets differ from the Polars examples. The algorithm run
omits workloads whose independent reference flow would exceed its work budget,
and the largest inputs have narrower test coverage. Single Polars medians moved
by up to 22% between repeat runs of the same build.

<span id="reproduce-and-data"></span>

<span id="operation-specific-commands"></span>

**Reproduce**

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity --locked > target/capacity-local.csv
uv run --no-sync python benchmarks/capacity_temporal.py > target/capacity-temporal-local.csv
```

`CAPACITY_BENCH_MAX_N`, `CAPACITY_BENCH_SAMPLES` and `CAPACITY_BENCH_FAMILY`
restrict the core run. Run both on a clean, committed tree after the
[release rebuild](benchmarking.md#setup).

[Setup, metrics and publishing](benchmarking.md) ·
[Rust samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow-core.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow-polars.csv) ·
[Run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow.metadata.json) ·
[Earlier core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.csv) ·
[Earlier Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-temporal-windows.csv) ·
[Omission log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-windows.log) ·
[Cleanup samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-cleanup-windows.csv) ·
[Design and historical validation](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection)

<span id="underlying-algorithm-comparisons"></span>

<span id="historical-validation"></span>

**History**

*Shared flow engine.* Constant capacity now runs on the same flow code as
[capacity profiles](capacity-profile-benchmarks.md), as one capacity segment.
Results and ties did not change: an exact comparison with the previous code,
revision `8888e96`, matched on more than 300 million random problems, including
every error. The measured shared-engine build is revision `a0d5912`.

**Rust algorithm only · median of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow.metadata.json)**

--8<-- "docs/assets/benchmarks/capacity-flow-core-table.md"

Across 473 workloads of at least 1 ms, the shared engine took 1.00× the separate
engine's time, both as a geometric mean and as a median. Single workloads ranged
from 0.60× to 1.40×, about as far apart as two runs of one build. Inputs with few
distinct endpoints stay slightly slower: identical intervals have a median ratio
of 1.07×. Peak requested heap rose by at most 2%, and fell from 96.5
to 56.6 MiB on the million-row overlap-set example at capacity 2, because workers
no longer copy rows. These are algorithm allocations, not whole-process memory.

**Complete Polars queries · median of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-flow.metadata.json)**

--8<-- "docs/assets/benchmarks/capacity-flow-polars-table.md"

Complete query times moved by 0.80× to 1.27× across 96 workloads of at least
1 ms, with a geometric mean of 0.98×. Repeat runs of one build also moved by up
to 22%, so this comparison shows no reliable change.

*First-path shortcut.* The shared engine keeps one special case for constant
capacity: an initial pass over the graph supplies the first route, so the solver
skips one shortest-path search. This table compares it with a
[candidate without the shortcut](https://github.com/jplauri/polars-intervals/commit/a914f5828f95b57ae24e4e78c0215cc5b8d298ea), in two alternating pairs of runs.

--8<-- "docs/assets/benchmarks/capacity-fast-path-table.md"

Across 54 flow workloads of at least 1 ms at capacity 2, the candidate took a
geometric mean of 1.44× and 1.45× longer in the two pairs. Capacities 3 and 4
took 1.24× longer, and the cost fades to about 1.0× above capacity 16. Complete
Polars queries took 1.05× longer, so the shortcut stays.

The earlier core and Polars runs record base revision
`cd3a0181232bda9a2c1f7b0aafdfef1adcd8233f`. The measured package
version was not recorded. The original core source hash matches `29d75ee`,
before `cee8e52` removed the per-interval edge-index vector and intermediate
worker-result buffering. Those changes affect allocations and reconstruction
inside the timed solver. The separately recorded cleanup run measures core
calls, not full Polars queries.

**Rust algorithm only · package uses up to 8 workers · median of 3 samples ·
[measurement settings](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-environment.json)**

These inputs are shuffled and have positive weights. The package can solve
independent overlap sets in parallel. The alternatives solve those sets one at
a time, solve the entire input together, or use a general-purpose optimizer.
All return an exact optimum.

--8<-- "docs/assets/benchmarks/capacity-core-table.md"

One million rows in independent overlap sets take 128 ms at capacity two.
A dense overlapping dataset of the same size takes 1.09 seconds. Parallel
processing helps independent sets, but the alternatives win on some dense or
identical-interval inputs.

On the million-row independent-set example, peak requested heap storage is
96.4 MiB for the package versus 56.6 MiB when processing sets one at a time.
The extra memory helps reduce runtime. These are algorithm allocations, not
whole-process memory.

The displayed run predates a memory-allocation cleanup. A separate before/after
run is retained, but does not establish a speedup from that change. Details and
omissions are in the [supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#scalar-capacity-selection).

</details>
