# Variable-capacity selection: design and experiments

The operation selects a globally maximum-weight feasible subset of fixed,
half-open, unit-demand intervals. It uses integer objective arithmetic and a
compact timeline. Reproduce the core experiments with:

```powershell
cargo bench -p intervals-core --bench max_weight_with_capacity_profile --locked
```

`PROFILE_BENCH_MAX_N`, `PROFILE_BENCH_MIN_N`, `PROFILE_BENCH_MIN_M`,
`PROFILE_BENCH_SAMPLES`, `PROFILE_BENCH_FAMILY`, and `PROFILE_BENCH_METHODS`
restrict a run. Defaults are one million jobs, three samples, and all families
and methods, with no lower job-count or segment-count restriction.
The harness requires release mode. Run
`benchmarks/capacity_profile_temporal.py` against the locally built release wheel.
It performs no optimization in Python.

## Normalization and timeline

Sort profile records by `(start, end)`, validate every record, discard validated
empty records, reject overlaps, insert zero-capacity gaps, and coalesce adjacent
equal values. Capacity outside the supplied profile is zero. Clamp capacities
to job count, and again to positive nonempty candidate count for local networks.
At most that many unit-demand jobs can be selected, so clamping cannot remove
a feasible subset. Polars endpoint logical dtypes must match exactly.

The timeline is the sorted union of candidate job endpoints and profile
endpoints intersecting their horizon. No elapsed physical ticks are generated.
Let `c_j` be capacity on `[t_j, t_(j+1))`. Positive empty jobs are selected
separately; zero and negative weights are omitted as in the scalar API.
Original row IDs are retained for reconstruction.

Packed normalization copies records before sorting. Indirect normalization
sorts indices and reads original records directly into the same normalization
scan, without a second intermediate record copy. Both produce identical
normalized segments, including internal zeros.

## Three exact formulations

### A: capacity-change transshipment

At node `j`, define supply `delta_j = c_j - c_(j-1)`, extending capacity by
zero before and after the horizon. Add forward timeline arcs with capacity
`c_j` and cost zero, and forward job arcs with capacity one and cost `-weight`.
Connect a super-source to positive supplies and negative supplies to a
super-sink. Send all supply.

**Cut invariant.** Sum conservation over timeline nodes `0..=j`. Supplies
telescope to `c_j`. All original arcs point forward, so the only outgoing
original flow across that cut is unused timeline flow plus selected job flow:

```text
unused timeline flow_j + selected jobs crossing segment_j = c_j.
```

Nonnegative unused flow proves feasibility. Conversely, any feasible subset
extends to a transshipment by assigning `c_j - selected_concurrency_j` to each
timeline arc. This extension respects capacities and balances. Only job arcs
have nonzero costs, so minimizing cost maximizes selected weight. Integral
supplies and capacities give an integral optimum; each job arc is Boolean.

### B: fixed K with lower bounds

Let `K = max(c_j)`. Send exactly `K` units from the first to last node, or
equivalently add a fixed return arc with lower and upper bound `K`. Give each
timeline arc lower bound `K-c_j` and upper bound `K`; job arcs remain unchanged.
Every cut carries `K`, so timeline flow at least `K-c_j` is equivalent to
selected concurrency at most `c_j`.

Subtract lower bounds and transfer their incoming/outgoing imbalances to
endpoint supplies. Timeline residual capacity becomes `K-(K-c_j)=c_j`.
The return arc and adjacent lower bounds give supply `delta_j` at every node.
Thus correctly transformed B is **exactly A**. The benchmark independently
constructs B from lower bounds to check this derivation and measure construction
cost. There is no reason to retain duplicate production code.

### C: backward-timeline circulation

Create backward timeline arcs `t_(j+1) -> t_j` with capacity `c_j` and zero
cost, together with forward negative-cost job arcs. Conservation across a cut
equates selected job flow to backward timeline flow, enforcing the same limit.
Every feasible subset extends to this circulation. The original graph needs
no baseline `K` flow.

The reference uses an exact standard reduction, not cycle cancellation:
initially saturate every negative-cost job arc. Each job end has one unit of
excess incoming flow and each start one unit of deficit. Restore conservation
by sending excess to deficits through backward timeline arcs and positive-cost
reverse job arcs. Initial residual costs are nonnegative. Reversing a job arc
rejects that initially selected job. Every feasible final circulation is
represented, and the mathematical constant initial cost is negative total
candidate weight. The implementation never accumulates that constant: it could
overflow `i128` even when the selected optimum fits. Only necessary shortest-path
arithmetic and the final selected objective are checked. Ordinary exact min-cost
flow solves the correction problem.

### Solvers and optimality

Production A and C share compact adjacency storage and exact SSP. A uses timeline-specific initial
potentials in `capacity_profile.rs`. It augments shortest residual paths with
a binary heap, exact `i128` costs, and feasible reduced-cost potentials. Every
augmentation sends the full path bottleneck. The benchmark compares this
implementation (forced `csr` for A and `csr_C` for C) with conventional
`Vec<Vec<Arc>>` SSP (`A`), and uses the latter for independently constructed B
and C. `csr_adaptive` invokes the final general solver without public fast paths.
Generic initial potentials
come from Bellman-Ford; A/B initially form a DAG and C starts with nonnegative
costs. There is no external graph dependency.

The reduced-cost and SSP optimality argument is the standard min-cost-flow
argument in [MIT 6.854 lecture notes](https://courses.csail.mit.edu/6.854/20/Notes/n09-mincostflow.html).
Lower-bound and supply conventions are documented in the
[LEMON minimum-cost-flow tutorial](https://lemon.cs.elte.hu/pub/doc/1.3/a00005.html).
These references support transformations; performance claims below come from
this repository's measurements.

### Tightening capacities before choosing a formulation

Raw supply magnitude alone is a poor dispatch predictor: a high-capacity spike
can carry mostly unused flow, while a tight bottleneck elsewhere restricts every
job spanning it. Before choosing A or C, compute two necessary capacity bounds.
If `starts_j` counts candidate starts at node `j`, selected concurrency can rise
by at most `starts_j`; it can fall by at most the candidate ends at a boundary.
Starting with zero outside the horizon, perform:

```text
forward:  bound_j = min(c_j, bound_(j-1) + starts_j)
backward: bound_j = min(bound_j, bound_(j+1) + ends_(j+1))
```

Induction shows that every originally feasible subset satisfies both bounds.
The bounds only decrease capacities, so feasible subsets are exactly preserved.
Forward positive changes sum to at most candidate count. The backward pass
preserves this property: whenever it lowers a segment, that value remains at
least the next segment's bound, so it creates no new forward rise there.

Keep these tighter capacities only if their positive-change supply is smaller
than the original supply. This avoids introducing pointless fluctuations into
an already cheap profile. Choose C when its positive endpoint imbalance is
less than one quarter of A's resulting supply; otherwise choose A. Both use
the same graph storage, shortest-path solver, and reconstruction framework.
The conservative factor avoids changing formulation for near-equal budgets;
it is a practical choice, not a universal runtime prediction.

The tightened model still reconstructs the original A cut invariant: add
`original_capacity_j - tightened_capacity_j` to unused timeline flow. The
selected-job flow is unchanged. Tests check both working-network conservation
and feasibility under the original supplied profile.

## Fast paths and decomposition

Empty input returns an empty mask after validation. An empty or globally zero
profile permits only positive empty jobs. Constant effective capacity over the
job horizon dispatches to the existing scalar kernel, including its specialized
capacity-one weighted scheduling algorithm. The scalar API is unchanged.

The general path filters jobs crossing zero capacity. A prefix count of zero
atomic segments supports constant-time tests after endpoint lookup, rather
than scanning each lifetime. An event-difference sweep selects all remaining
positive jobs immediately when they fit together.

Regions with no candidate crossing their shared boundary are independent
overlap components. Solving each maximizes the sum of their optima. Zero gaps
can create further components after impossible jobs are removed. The harness
compares one global network, serial components, and eight scoped workers.
Results scatter by original row ID, independent of worker completion order.
Benchmark-only decomposition candidates force component flow, measuring it
separately from feasibility-sweep savings.

## Correctness gates and measurement protocol

Before timing, 256 seeded small arbitrary instances are checked against an
independent exhaustive subset oracle. Every exact candidate must return a
Boolean mask of the right length, satisfy independent profile feasibility, and
attain the same objective. The oracle directly enumerates subsets and counts
coverage on atomic segments, sharing no graph or flow code. Every workload
also checks a restriction of at most ten jobs exhaustively. Every warm-up,
allocation run, and timed output is validated. Tied masks need not match.

`tests/capacity_profile_formulations.rs` adds proptest comparisons for all three
formulations and filtering/decomposition alternatives. It generates up to ten
jobs on `[-6,10]`, weights in `[-10,20]`, and unit-segment capacities in `[0,4]`,
omitting zero rows and reversing profile order. Production API and normalization
have separate, larger property suites.

The matrix uses orthogonal slices, not an unbounded Cartesian product:

- Jobs: 1K, 10K, 100K, 1M. Large constrained global comparisons stop at 10K;
  component and fast-path scaling continues through 1M.
- Profiles: 1, 4, 16, 100, 1K, 10K, and empty.
- Capacities: 1, 2, 4, 8, 16, 64, 256, 1000, one billion, zero, and values
  exceeding all candidate concurrency.
- Patterns: constant, single bottleneck, increasing, decreasing, sawtooth,
  frequent one-unit changes, zero gaps, high capacity, and globally zero.
- Jobs: disjoint, sparse, moderate, dense, nested, identical, touching, many
  empties, mostly negative weights, one valuable long job, short jobs near
  bottlenecks, mostly outside the profile, spanning gaps, and many components.
- Order: sorted, shuffled jobs, shuffled profile, both shuffled.
- Native adapters: Int64, Date, Datetime microseconds, UTC-aware nanoseconds.
  Integration tests cover all supported temporal units.

Input generation and verification are untimed. Production, scalar, and CSR
candidates include extracting columns from packed jobs; generic candidates
use those records directly. Thus production-versus-generic measurements
conservatively include extra linear input-copy cost. Scalar-versus-profile
comparisons share this overhead. Compare profile normalization candidates using
`normalization_ns`, avoiding unrelated mask construction overhead.

Three randomized-order samples follow verified warm-ups. A separate untimed
run records requested heap peak and allocation count, excluding inputs and
verification. This is **not process RSS**. Parallel phase times sum workers'
durations; `ns` is elapsed wall time and determines speed. The public production
and scalar wrappers are not instrumented internally: their phase and graph
fields are encoded as zero, meaning unavailable, not necessarily no flow.
Forced CSR and `csr_adaptive` expose actual phases and graph metrics; timed
public calls still report full runtime, verified objective, and allocation peak.

Raw samples and environment metadata live in `benchmarks/results/`.
`benchmarks/capacity_profile_summary.py` computes medians preserving workload
identifiers. No equivalent exact native-Polars operation was identified, so no
synthetic dataframe baseline is claimed. Joins and event sweeps can construct
inputs or check feasibility, but do not by themselves optimize the global
combinatorial objective.

## Complexity and capacity magnitude

For `n` jobs and `p` profile rows, normalization takes `O(p log p)` time and
`O(p)` space. Candidate sorting and timeline construction take
`O((n+p) log(n+p))` time and `O(n+p)` space. A constrained component with `V`
vertices, `E` edges, and `a` augmentations takes `O(a E log(E+1))` time and
`O(E+V)` space after construction. Sum over components for the decomposed
bound. Heap entries can scale with parallel job arcs, so `log(E+1)` is safe
even with few distinct endpoints.

A sends `F_A=sum(max(delta_j,0))` integral units in at most `F_A` augmentations.
A residual path containing a job arc has unit bottleneck; unused-resource paths
can send many units at once. Capacities are clamped to candidate count, so raw
billion-sized values cannot introduce billion-step loops. Raw A's supply can
grow with profile changes, which the forced reference measurements expose.
Production capacity tightening bounds its chosen A supply by candidate count;
C's net excess from saturated jobs is also at most candidate count. Therefore
the final production solver performs at most `n_c` augmentations per component,
independent of raw capacity magnitude and profile-change count. Its worst-case
bound is `O(n_c E_c log(E_c+1))`, plus sorting and linear preprocessing. C can
still be substantially faster than A within that bound on frequent changes;
A avoids rejecting almost every job in dense low-capacity instances.

All network storage is linear in compact breakpoints plus candidate jobs.
There is no pairwise conflict graph. Scoped parallelism can multiply live
scratch buffers; serial solving releases one graph before constructing the next.

## Recorded release results

Measurements use Windows x64, an AMD Ryzen 9 3900X (12 cores/24 logical CPUs),
32 GiB installed memory, Rust 1.98.1, and optimized Cargo bench builds. No
repository builds or tests ran concurrently with the controlled slices.
Values below are medians of three timed samples, excluding the separate
allocation pass. Memory is decimal MB of requested heap, not RSS. Raw CSVs and
environment metadata are authoritative. Development measurements below explain
the production choices; the final matrix records the completed implementation.

### Development decisions

| Controlled workload | Forced A | C | Implication |
| --- | ---: | ---: | --- |
| 1K dense jobs, 16 segments with one capacity-1 bottleneck and capacity 1000 elsewhere | CSR 290.49 ms, 1755 augmentations | CSR 165.29 ms, 1000 augmentations | Guarded tightening plus adaptive choice: 0.513 ms, one augmentation |
| 1K dense jobs, 100 alternating capacity 1/1000 segments | CSR 182.58 ms, 1149 augmentations | CSR 163.22 ms, 1000 augmentations | Guarded tightening plus adaptive choice: 0.510 ms, one augmentation |

These cases motivated the capacity bounds: raw supply counts alone do not
predict how much unused flow can traverse bottlenecks.
`capacity-profile-adversary-*.csv` labels the original supply-only
selector `csr_budget` and the independently tightened benchmark candidate
`csr_tight`. Both remain benchmark-only for reproducing the decision.

| Independent components, 32 jobs each | Global forced CSR | Serial component CSR | Eight workers |
| --- | ---: | ---: | ---: |
| 1K jobs | 1.936 ms | 0.660 ms | 0.642 ms |
| 10K jobs | 22.252 ms | 6.385 ms | 2.330 ms |

Larger constrained global graphs were omitted after measuring this trend;
the final component scaling table below reaches 1M jobs. These development
slices precede final adaptive dispatch: their raw `production` columns describe
the earlier baseline. All raw samples remain available.

### Final production matrix

The completed core run contains **9,699 timed samples across 187 workloads and
20 candidate/policy labels**. The installed release-native run contains **240
timed samples**. Every output passed the independent checks described above.
The core matrix took ten minutes; compilation and repository tests were stopped
throughout both final timing runs. Source hashes, toolchain details, and timing
boundaries are recorded in
`benchmarks/results/capacity-profile-environment.json`.
The checked-in raw files are `benchmarks/results/capacity-profile-core.csv` and
`benchmarks/results/capacity-profile-temporal.csv`; earlier decision slices are
identified separately in the environment metadata.

| 10K moderate jobs, capacities alternating 4/5 | Generic A | Generic B | Generic C | Forced CSR A | Forced CSR C | Final public API |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1K profile segments | 386.96 ms | 383.66 ms | 39.65 ms | 385.94 ms | 39.53 ms | 37.90 ms |
| 10K profile segments | 3803.96 ms | 3786.62 ms | 41.14 ms | 3820.30 ms | 40.29 ms | 37.29 ms |

At 10K profile segments, raw A makes 5,004 augmentations versus C's 32.
Production uses C here and is about 102 times faster than forced raw CSR A.
B's lower-bound construction becomes the same graph as A and offers no distinct
solver advantage. Retaining A and C behind one solver is justified by their
different regimes; retaining separate production B code is not.

| 32-job components, alternating capacities 4/5 | Public API | Forced serial components | Forced eight workers | Public peak heap |
| --- | ---: | ---: | ---: | ---: |
| 1K jobs | 0.620 ms | 0.648 ms | 0.714 ms | 0.144 MB |
| 10K jobs | 5.738 ms | 6.016 ms | 2.246 ms | 1.718 MB |
| 100K jobs | 21.118 ms | 65.013 ms | 20.257 ms | 15.593 MB |
| 1M jobs | 226.258 ms | 671.112 ms | 216.676 ms | 143.333 MB |

The final 1M forced serial/parallel peaks are 125.372/127.608 MB; these exclude
some public-wrapper preprocessing and must not be mistaken for the public
143.333 MB peak. Parallelism is useful for substantial separated workloads;
the 1K result demonstrates its overhead on small inputs. The conservative
16,384-row threshold keeps the 10K public call serial. Parallel solving also
requires at least eight components and uses at most eight workers. Each worker
releases its component graph before constructing the next.

### Measured fast paths

| Workload | Comparison | Final result |
| --- | --- | ---: |
| 1K disjoint jobs, 16 segments alternating 4/5 | Forced CSR without / with all-positive sweep | 0.888 / 0.082 ms |
| 1K dense jobs crossing zero-capacity gaps | Forced CSR without / with impossible-job filtering | 1.729 / 0.225 ms |
| Same zero-gap workload | Public filtering followed by feasibility sweep | 0.093 ms |
| 1M jobs, all-zero profile partly covering horizon | Public zero dispatch / existing scalar zero | 9.350 / 9.759 ms |
| 1M jobs, empty profile | Public empty-profile dispatch / scalar zero | 9.451 / 9.318 ms |

Impossible-job filtering is not free: on the 1K disjoint no-zero workload,
forcing that preprocessing increased raw CSR time from 0.888 to 0.937 ms.
Its large gain when jobs cross gaps supports retaining the compact prefix
filter. The all-positive sweep removes the entire network on feasible inputs.
The global-zero check reduces the 147 ms preprocessing case recorded in
`capacity-profile-zero.csv` to about 9 ms; positive empty jobs remain selected.
Empty-job inputs also appear in the timed matrix, but sub-microsecond
differences are not useful decisions.

| Constant profile | Profile API | Existing scalar | Forced general CSR A |
| --- | ---: | ---: | ---: |
| 1K moderate jobs, capacity 1 | 0.0148 ms | 0.0132 ms | 0.2076 ms |
| 1K moderate jobs, capacity 4 | 0.4355 ms | 0.4226 ms | 0.5738 ms |
| 100K disjoint jobs, 16 equal segments | 3.1384 ms | 2.9519 ms | intentionally omitted |
| 1M disjoint jobs, 16 equal segments | 44.5537 ms | 42.1636 ms | intentionally omitted |

Constant dispatch preserves the existing capacity-one specialization and costs
about 1.6 microseconds in that 1K case. At 1M jobs, validating and normalizing
the profile adds about 5.7% over the scalar call; both avoid flow. Public peak
heap is 83.332 MB. Dense 1K single-bottleneck cases with capacities 64, 256,
1000, and one billion all require **one** augmentation after tightening, with
forced final-general times 0.337–0.344 ms. Raw capacity magnitude does not
produce unit-by-unit augmentation loops.

Final profile-normalization measurements at 10K rows:

| Profile order and structure | Packed | Indirect | Indirect heap saving |
| --- | ---: | ---: | ---: |
| Shuffled, high capacity | 307.1 microseconds | 341.7 microseconds | 160 KB |
| Sorted, alternating 4/5 | 97.8 microseconds | 78.3 microseconds | 160 KB |

Packed sorting remains the single production implementation: it wins the
unsorted input that needs sorting, while the indirect advantage on sorted
records is only about 20 microseconds in this slice. Both are linear-space;
keeping a second normalization policy for that small saving would add dispatch
and maintenance work without changing the constrained solver bottleneck.

### Installed native endpoint measurements

The temporal harness uses a release wheel built from the source distribution,
installed outside the checkout with Python 3.14 and Polars 1.44.2. Each group
contains up to 32 mutually overlapping jobs, so summing the largest weights
within each clique gives an independent exact objective oracle. Variable
capacities cycle through 2/4/6/8, with zero gaps between groups. Date and timestamp values are
translated to real calendar ranges; timeline size depends only on endpoints.

| One million jobs | Constant profile | Scalar constant | High profile | Variable profile |
| --- | ---: | ---: | ---: | ---: |
| Int64 | 77.565 ms | 88.752 ms | 46.718 ms | 160.231 ms |
| Date | 56.543 ms | 53.554 ms | 31.178 ms | 146.407 ms |
| Datetime[us] | 71.046 ms | 66.708 ms | 43.267 ms | 160.077 ms |
| Datetime[ns, UTC] | 67.595 ms | 66.517 ms | 42.008 ms | 159.262 ms |

The variable Int64 workload takes 0.334, 3.217, 14.280, and 160.231 ms at
1K, 10K, 100K, and 1M jobs. These are complete native API measurements,
including Polars extraction and output construction. The scalar API is an
expression evaluated through a query, whereas the profile API accepts two
eager frames directly; their overhead is not identical. Use the core constant
table above to assess dispatch overhead on otherwise matching wrappers.

## Validation

The final implementation passed workspace formatting, all 253 Rust tests and
doctests, Clippy across all targets with warnings denied, and Rustdoc with
warnings denied. The stronger targeted run used 4096 generated cases for the
internal profile/flow properties; the public property suite and independently
implemented formulation oracle also passed. Existing benchmark plot tests and
figure generation passed. The locked Python suite passed all 1834 tests and
doctests, including 273 profile integration cases; lock consistency and Ruff
lint/format checks passed. A release wheel
built from the source distribution and installed outside the checkout passed
all 1834 Python tests and doctests on both minimum Polars 1.44.1 and locked
Polars 1.44.2. Twine's strict package metadata checks and source-distribution /
wheel consistency checks passed.
