# Variable-capacity selection benchmarks

[All benchmarks](benchmarks.md) · [Measurement guide](benchmarking.md)

## Summary

[`max_weight_with_capacity_profile`](api.md#polars_intervals.max_weight_with_capacity_profile)
selects maximum-weight intervals under a changing capacity profile. Production
chooses between two exact flow formulations and exploits independent components;
frequent capacity changes can make that choice decisive. Profile normalization
and conservative parallel thresholds still add overhead in some measured cases.

## Results

**Complete Polars API · solver uses up to eight workers · Polars pool size unrecorded
· median of 3 samples · [run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json)**

--8<-- "docs/assets/benchmarks/profile-polars-table.md"

Each clique contains at most 32 jobs; variable profiles have zero gaps between
cliques. Timings include eager API extraction and output construction, excluding
fixture creation and casts. The separate scalar-expression measurements have a
different query boundary and do not isolate profile dispatch overhead.

**Rust core · single-threaded formulation comparison · median of 3 samples ·
[run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json)**

Sorted jobs and profile segments, with capacities alternating 4/5. Forced
candidates include phase clocks; production is uninstrumented:

--8<-- "docs/assets/benchmarks/profile-formulations-table.md"

Both references use compact adjacency storage. Transshipment handles capacity
changes as supplies; circulation starts with jobs selected and corrects excess
flow. Production chooses circulation for these frequent-change cases. Its
capacity tightening also prevents large raw capacities from causing unit-by-unit
augmentation loops.

**Rust core · production serial below 16,384 rows, up to eight workers above ·
median of 3 samples · [same final run metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json)**

Sorted component workloads, capacities alternating 4/5:

--8<-- "docs/assets/benchmarks/profile-components-table.md"

Forced workers lose at 1K but beat production's conservative serial policy at
10K. At 1M, production's peak **requested live heap** is 143 MB; forced serial
and parallel candidates record 125/128 MB, excluding some public-wrapper
preprocessing. Those memory totals do not isolate the cost of parallelism.

Constant profiles delegate to the scalar solver. On 1M sorted disjoint jobs
with 16 equal-capacity segments, the profile/scalar core calls took 44.6/42.2 ms:
normalization remains a measured cost even when flow is bypassed.

## Coverage and limitations

The final core run covers 187 workloads and 9,699 samples: job geometry, profile
patterns and order, zero gaps, signs of weights, segment count and capacity
magnitude. Constrained global comparisons stop at 10K jobs; components and fast
paths extend to 1M. These are selected slices, not a full Cartesian matrix.
No equivalent exact native Polars baseline is provided. The release wheel covers
Int64, Date, microsecond Datetime and UTC nanoseconds on clique fixtures.

Public, scalar and compact-storage candidates include copying packed jobs into
columns; generic references consume packed jobs directly. Public wrappers have
no internal phase clocks; their zero phase/graph fields mean unavailable.
Forced candidates retain instrumentation. Small timing differences across these
boundaries do not establish a reliable winner.

Independent exhaustive subsets validate small cases and workload restrictions;
full workloads check feasibility and exact-candidate objective agreement.
Temporal cliques have an independent top-k oracle. Earlier development slices
contain historical `production` implementations and are **not** the final run
shown here; their derivations, checks and contradictory results remain in the
[design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#variable-capacity-selection).

## Reproduce and data

<details markdown="1">
<summary>Operation-specific commands</summary>

```sh
cargo bench -p intervals-core --bench max_weight_with_capacity_profile --locked > benchmarks/results/capacity-profile-local.csv
uv run --no-sync python benchmarks/capacity_profile_summary.py benchmarks/results/capacity-profile-local.csv
python -I /path/to/checkout/benchmarks/capacity_profile_temporal.py > capacity-profile-temporal-local.csv
```

`PROFILE_BENCH_MAX_N`, `PROFILE_BENCH_MIN_N`, `PROFILE_BENCH_MIN_M`,
`PROFILE_BENCH_SAMPLES`, `PROFILE_BENCH_FAMILY` and `PROFILE_BENCH_METHODS`
restrict the core run. Run the temporal command with the installed release
wheel's Python from outside the checkout, following the shared setup.

</details>

[Setup, metrics and publishing](benchmarking.md) ·
[Final core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-core.csv) ·
[Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-temporal.csv) ·
[Metadata and historical run inventory](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/capacity-profile-environment.json) ·
[Design notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/selection-notes.md#variable-capacity-selection)
