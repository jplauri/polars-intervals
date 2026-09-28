# Lane balancing benchmarks

## Summary

The [balanced constructor](usage.md#balance-lane-row-counts) reduced row-count
spread `D` on **108 of 132 scaling instances**, reaching the certified optimum on
all 24 planted cases. It preserves minimum lane count and never worsens `(D,Q)`
against existing `assign_lanes`, with additional runtime that can be substantial.
The default 100,000-unit budget polishes the best baseline/forward/backward seed;
balance remains heuristic.

These runs predate interface consolidation. The recorded “repair” method now
uses `assign_balanced_lanes(..., initial_lanes=...)`. Algorithms are unchanged;
saved measurements, hashes and verification records retain their original provenance.

## Results

Polars collection · one thread · one warmup, median of five samples ·
[quality-run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.metadata.json)

Measurements include lazy optimization, endpoint/lane validation, preprocessing,
search and materialization. Constructing repair's original baseline, input
preparation, independent validation and final Series destruction are outside
timing. Each repeat repairs the same original coloring.

--8<-- "docs/assets/benchmarks/balance-polars-table.md"

| Same workload | Baseline D | Repaired D | Balanced D |
| --- | ---: | ---: | ---: |
| Interior empties | 9 | 1 | 1 |
| Late clique, 100,000 rows | 99,992 | 99,992 | 0 |
| Forced star, 100,000 rows | 99,998 | 99,998 | 99,998 |
| Nearly clique, 100,000 rows | 49,999 | 49,999 | 49,999 |

These [quality records](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.quality.csv)
show both gains and cost without improvement. The late-clique constructor reaches
equity at 4.13× baseline runtime; the nearly-clique case costs 17.4× with unchanged
balance. Ratios mean candidate median / existing-production median; above one is
slower. The simultaneous-flip supplied assignment improves `(19,15)` to `(17,17)`.

Rust core · two warmups, median of five samples ·
[core metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-core-windows-20260928.metadata.json)

Core calls include validation, copying, seed preparation and output construction;
output destruction is excluded. Requested-live-heap peaks are measured separately
with allocator tracking enabled, including the output but excluding caller inputs,
allocator overhead, stack and process RSS. Tracking is disabled during timing.

--8<-- "docs/assets/benchmarks/balance-core-table.md"

Across 60 core fixtures, 100,000 units improved constructor quality over 10,000
units in four cases; 1,000,000 units improved two further cases. Splitting the
default budget across three seeds won zero cases and lost three. Individual
forward/backward seeds can regress: both gave `D=874` on 1,000-row stars against
baseline `D=6`. Retaining the baseline prevents that loss. Full
[budget/quality records](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-core-windows-20260928.quality.csv)
justify the conservative default without claiming quality saturation.

## Coverage and limitations

The matrix covers both smoke seeds, exact/scaling suites, regressions and separate
tiny JAIST all/connected populations. `exact` names a corpus, not an oracle result.
Capped exhaustive oracles, planted certificates and equity witnesses establish
known optima; other gaps stay unknown. Independent concurrency/per-lane checks
validate every output. Quality counts instances, not timing repeats, and
overlapping corpora are not pooled. Baseline repair hit only 12/24 certified
scaling optima; its worst additive gap was 711, versus zero for construction.

A separate run checks Date, timezone-aware Datetime, four-group windows, slices,
unequal chunks, and auto/streaming collection. These order-preserving variants
exercise execution semantics; they do not create independent graph populations.
Large cliques stop at equity. Large nearly-clique cases retain bounded search.
Public Polars results expose no work counters: non-equitable positive-budget stops
remain `not_exposed`. Core diagnostics distinguish completed pairwise fixed points
from budget/scratch stops.

For three or more lanes, pairwise optimality does not imply globally optimal
balance: the supplied `(15,16,17)` gadget coloring remains stuck despite a verified
`(16,16,16)` witness. Optional component relabeling and three-color search are absent.
Full JAIST downloads, other platforms and million-row runs were not measured here.
See the [shared methodology](benchmarking.md) and
[implementation notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/balance-lanes-notes.md)
for exact budget units, oracle caps, timing scopes and limitations.

## Reproduce and data

<details markdown="1">
<summary>Reproduce this operation</summary>

Follow the [release-plugin setup](benchmarking.md#setup). Generate the existing
corpora first, then run the complete commands in the
[supporting notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/balance-lanes-notes.md#reproduce-the-recorded-matrix).
A small offline run is:

```sh
python benchmarks/generate_interval_graphs.py synthetic --suite smoke --seed 42 --output target/balance-smoke
uv run --no-sync python benchmarks/balance_lanes.py --dataset target/balance-smoke --handcrafted --max-work 100000 --warmups 1 --repeats 5 --output benchmarks/results/balance-local-new
cargo bench -p intervals-core --bench balance_lanes --locked
```

Set `POLARS_MAX_THREADS=1` before Python. Use fresh output prefixes and run
benchmarks sequentially after compilation; runner metadata records native/source
hashes, dirty revision, settings and corpus hashes. The core environment variables
in the supporting notes select the recorded size/budget matrix.

</details>

Saved [quality](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-quality-windows-20260928.summary.json),
[scaling](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-scaling-windows-20260928.summary.json)
and [execution-variant](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-polars-workloads-windows-20260928.metadata.json)
records link raw samples, provenance and source hashes. The
[verification log](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/balance-verification-windows-20260928.md)
records executed checks. Generated shards stay outside version control; table
downloads retain sample ranges and exact medians.
