# Selection algorithm design notes

Supporting derivations and development evidence for the compact benchmark reports.
The reports link saved measurements and describe their timing scopes; these notes
explain implementation choices. Historical runs below are identified separately
from the final measurements.

## Variable-capacity selection

### Normalization and timeline

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

### Three exact formulations

#### A: capacity-change transshipment

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

#### B: fixed K with lower bounds

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

#### C: backward-timeline circulation

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

#### Solvers and optimality

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

#### Tightening capacities before choosing a formulation

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

### Fast paths and decomposition

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


### Complexity and capacity magnitude


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


### Development evidence and rejected alternatives

The [run inventory](results/capacity-profile-environment.json) separates development
slices from the completed matrix in [capacity-profile-core.csv](results/capacity-profile-core.csv).
Early files' `production` labels describe the earlier implementation. They are
not repeat measurements of the final dispatcher.

The adversarial slices motivated capacity tightening. With 1K dense jobs and a
single capacity-one bottleneck among 16 capacity-1000 segments, forced compact
transshipment took 290.49 ms/1,755 augmentations and circulation took 165.29 ms/
1,000 augmentations. Independently tightened adaptive flow took 0.513 ms/one
augmentation. Alternating 1/1000 capacities over 100 segments gave the same
qualitative failure: 182.58/163.22 ms for the forced formulations versus 0.510 ms
for tightening. See [dense](results/capacity-profile-adversary-dense.csv) and
[identical](results/capacity-profile-adversary-identical.csv) slices. `csr_budget`
is the original supply-only selector; `csr_tight` is the benchmark tightening
candidate. These labels remain available to reproduce the decision.

The early [component slice](results/capacity-profile-components.csv) measured
forced global/serial/eight-worker compact flow at 1.936/0.660/0.642 ms for 1K
jobs and 22.252/6.385/2.330 ms for 10K. Larger constrained global graphs were
omitted. Final production requires at least eight components and 16,384 rows
before using up to eight workers; this deliberately leaves the faster forced
10K parallel result on the table.

The final matrix records further tradeoffs:

- For 1K disjoint jobs and 16 alternating 4/5 segments, the all-positive
  feasibility sweep changes forced compact runtime from 0.888 to 0.082 ms.
  Forcing impossible-job filtering on this no-zero case instead increases it
  to 0.937 ms. On dense jobs crossing zero gaps, filtering cuts 1.729 to
  0.225 ms, and production filtering plus the feasibility sweep takes 0.093 ms.
- The earlier [zero-profile slice](results/capacity-profile-zero.csv) recorded
  roughly 147 ms of preprocessing; final global-zero dispatch takes roughly
  9 ms at 1M jobs. This is development evidence across recorded implementations,
  not a pooled run. Positive empty jobs are still selected.
- Packed versus indirect normalization at 10K profile rows takes 307.1/341.7 µs
  on shuffled high-capacity records, but 97.8/78.3 µs on sorted alternating
  records. Indirect saves 160 KB in either case. These are `normalization_ns`,
  not full-call comparisons. Production retains one packed policy.
- Dense 1K single-bottleneck cases with high capacities 64, 256, 1,000 and one
  billion all take one augmentation after tightening. Forced final-general
  medians span 0.337–0.344 ms. This tests capacity magnitude separately from
  profile-change count.

Raw phases need care: parallel phase values sum workers' durations, whereas
`ns` is wall time. Public production/scalar phase and graph fields are zero
because those wrappers are not internally instrumented. Compact, public and
scalar candidates include packed-job-to-column copies; generic references do
not. The final matrix retains all 20 candidate/policy labels, including forced
formulations and benchmark-only filters.

### Correctness evidence

Before timing, 256 seeded arbitrary small instances and each workload's at-most-
ten-job restriction are checked against independent exhaustive subset search.
That oracle directly counts selected coverage on atomic profile segments and
shares no graph code. Every timed result is checked for Boolean shape,
feasibility and objective agreement; tied masks need not match. Full large
non-structural instances are not exhaustively searched.

[Formulation properties](../crates/intervals-core/tests/capacity_profile_formulations.rs)
compare the three formulations and filtering/decomposition alternatives on up
to ten jobs, weights of both signs, omitted zero profile rows and reversed
profile order. Additional suites check normalization and the public API.
Internal flow checks cover residual conservation and reconstructed feasibility
under the original, untightened profile. Temporal cliques use an independent
sum-of-largest-weights optimum plus capacity checks.

Historical validation is recorded in the environment file: 253 Rust tests and
doctests, 1,834 Python tests/doctests, stronger 4,096-case internal properties,
and installed-wheel tests against Polars 1.44.1 and 1.44.2. These are validation
at the recorded revision, not checks rerun by editing these reports.

## Scalar-capacity selection

The specialized network has one vertex per distinct endpoint, capacity-`k`
zero-cost forward timeline edges, and capacity-one job edges of cost `-weight`.
Sending exactly `k` integral units enforces the overlap limit: at every cut,
selected interval flow plus unused timeline flow equals `k`. Conversely, any
feasible selection can be partitioned into `k` non-overlapping schedules, each
forming a source-to-sink path. Minimum cost is therefore negative maximum weight.
Reverse residual edges allow later augmentations to revise earlier choices.
Repeatedly fixing a capacity-one optimum cannot do that: intervals
`[0,2), [1,3), [2,4), [3,5)` with weights `3,2,2,3` yield only 8 by repeated
scheduling, while capacity two admits total weight 10.

Production stores paired residual edges contiguously with compact endpoint
indices and CSR adjacency. An initial DAG pass supplies the first shortest path
and feasible potentials; later paths use heap-based Dijkstra with exact reduced
costs and reusable scratch storage. Strict relaxation and `(distance, vertex)`
heap order make ties deterministic; augmentations send the full path bottleneck.
The independent generic reference instead uses conventional adjacency lists and
Bellman-Ford initialization.

Nonpositive jobs can be omitted, and positive empty jobs are selected separately.
Capacity zero validates rows without building a graph; capacity one delegates
to the existing weighted scheduler. A concurrency sweep bypasses flow when all
useful jobs fit. Components separate at `next_start >= max_end`, and their
optimum objectives add. Each component also checks its own concurrency.
Production uses up to eight scoped workers with at least eight components and
`positive_nonempty_rows * capacity >= 64,000`. Each worker owns local masks for
disjoint batches; results return to original row order without a shared network.
The threshold reflects these measurements, not a universal optimum.

Worst-case work is `O(n log n + sum(k * n_c * log(n_c + 1)))` over constrained
components, with `O(n)` extra space including output. Sufficient-capacity cases
cost `O(n log n)`; capacity zero is linear and capacity one uses the existing
`O(n log n)` scheduler. Serial decomposition retains one network at a time;
parallel batches add row and mask storage. Arithmetic widens integer weights to
`i128`, checks objective/cost overflow, and uses unsigned reduced distances for
large non-shortest detours. No floating-point objective approximation is used.

The original [core run](results/capacity-windows.csv) predates a cleanup removing
one per-interval edge-index vector and folding worker results directly into the
mask. The separate [cleanup run](results/capacity-cleanup-windows.csv) covers
330 workloads per revision: components through 1M and dense cases at 1K. For
1M positive shuffled component rows at capacity two, production allocations fell
from 437,664 to 406,414; whole-flow requested heap fell by 8,000,000 bytes.
All objectives matched. The unchanged generic engine also showed substantial
timing variation, so no cleanup speedup is claimed. Original and cleanup source
hashes remain separate in the [metadata](results/capacity-environment.json).

Before timing, each candidate is checked for feasibility and full objective
agreement. Every workload also compares a ten-row restriction against exhaustive
subsets; large structural cases use empty/sufficient-capacity or clique top-k
oracles. Large non-structural optima rely on agreement of independent flow engines.
Capacity-one checks compare the existing scheduler. Property tests exercise
exhaustive optimality, permutations, translations, capacity monotonicity,
component additivity and residual network invariants. See the
[core suite](../crates/intervals-core/tests/max_weight_with_capacity.rs).
Temporal tests verify independent clique top-k objectives and feasibility.

The original metadata retains historical validation: 91 Rust tests/doctests,
1,086 Python tests/doctests, 49 release-helper tests and isolated-wheel checks
on both recorded Polars versions. Other platform wheel builds remained CI work.

## Covering

The [shared covering methodology](covering-methodology.md) retains workload and
correctness inventories for both operations; raw [samples](results/covering-windows.csv)
include every candidate, phase and allocation measurement. Production solvers
were unchanged by the later harness/adapter cleanup, whose source hashes are
separate in [metadata](results/covering-environment.json).

Minimum-cardinality covering sorts packed clipped `(start, end, row)` records
and sweeps a frontier. Among intervals starting at or before the frontier,
choose the furthest end. Every scanned alternative ends no further, so it may
be discarded. Replacing an optimal cover's first advancing interval with this
one never increases the remaining interval count; repeating proves optimality.
Equal effective ends prefer original row index. The indirect candidate instead
sorts row indices; the heap provides a distinct greedy implementation. The
explicit sortedness check was rejected because gains were inconsistent with
the standard sort's existing ordered-input handling. Time is `O(n log n)` and
additional storage is `O(n)`, including output.

Minimum-cost covering processes candidates by increasing effective right end.
Compress right endpoints plus target start; each reachable frontier stores its
best `(cost, count, predecessor)`. A reversed Fenwick tree queries reachable
frontiers `x >= left`. Only frontiers strictly below the current right end have
been published; publish a batch only after querying every interval with that
same end. Each transition extends a continuous prefix. Every nonredundant
cover can be ordered by advancing right ends, and nonnegative costs plus the
secondary count objective make nonadvancing intervals unnecessary. This proves
the recurrence. Cheapest-first greedy fails: `[0,4):1`, `[0,6):5`,
`[4,10):100`, `[6,10):5` has optimum 10, but choosing the cheapest first can
cost 101.

Backpointers restore the original-row mask. Remaining ties use row index and
predecessor coordinate; a particular tied mask is not a public contract.
Nonnegative costs permit discarding overflowing paths because later additions
cannot restore representability. If no final state remains after overflow,
an unweighted feasibility check distinguishes infeasibility from an optimum
above `i128::MAX`. Both tree candidates stop ancestor updates when the minimum
is unchanged. The segment tree retains no redundant DP array. Fenwick storage
is `O(m + q + n)` for useful rows `m` and distinct frontiers `q <= m+1`; the
tree is released before allocating output. Contiguous physical endpoint columns
are borrowed, multiple chunks copied, and costs widened once to `i128`.

Historical validation counts, release artifacts, source hashes and cleanup
checks remain in the metadata. The exponential subset oracle belongs to small
tests; larger benchmark candidates cross-check objectives. The native suite
checks continuous coverage and deterministic masks, not an independent optimal
objective. Neither report claims otherwise.

## Minimum stabbing points

All candidates use the same greedy rule: process increasing ends and choose
`predecessor(end)` when the last point misses the current interval. Validation
rejects empty/reversed intervals in original order before sorting. Packed sort
copies endpoint pairs, indirect sort copies row indices, and the detection
candidate scans borrowed arrays when ends are nondecreasing. Production uses
that detection plus the packed fallback, without internal phase clocks.

For exactness, consider the earliest-ending uncovered interval. Any solution
contains a point `q` in it. Move `q` to `predecessor(end)`: any later-ending
interval containing `q` also contains the moved point. Thus an optimum containing
the greedy point exists; remove covered intervals and repeat. Triggering
intervals are pairwise disjoint, so every stabbing set needs at least one point
per trigger. This also proves equality between minimum stabbing number and
maximum disjoint interval packing.

Sorting costs `O(n log n)` and scanning `O(n)`; extra storage is `O(n + k)`
including `k` output points. End-sorted inputs take linear time and only `O(k)`
output space. Packed unsorted storage adds `2 * n * sizeof(endpoint)` bytes.
The adapter preserves physical width, borrows contiguous columns and copies
multiple chunks as necessary.

The [raw core matrix](results/stabbing-windows.csv) preserves the instrumented
packed, indirect and detection candidates alongside uninstrumented production.
Across the original matrix the detection/packed geometric mean median ratio was
0.403 on sorted inputs. Excluding equal-end/identical families, unsorted ratios
were 0.988 shuffled and 1.009 reverse. These are aggregates of per-case median
ratios, not paired-sample distributions or significance tests; the compact report
instead presents explicit representative workloads and the reverse-order losses.

The independent large-case oracle computes a right-to-left maximum disjoint
packing: sort starts descending, accept only when an interval's end does not
cross the last accepted start. It never computes stabbing points. Small oracles
independently enumerate subsets of all integer coordinates in the finite domain,
and subsets of intervals checked pairwise for disjointness; 256 small cases
cross-check both before timing. Each temporal result is checked with a coverage
as-of join and a family-specific disjoint-packing certificate.

See [core correctness tests](../crates/intervals-core/tests/minimum_stabbing_points.rs)
and [Python integration tests](../tests/test_minimum_stabbing_points.py) for
coverage, exact cardinality, determinism, permutations, translation, empty
infeasibility, integer extremes, groups, chunks and temporal metadata.
Nanosecond checks use physical values rather than Python datetime precision.
The [environment record](results/stabbing-environment.json) retains historical
125 Rust tests/doctests, 1,295 Python tests/doctests and installed-wheel checks.
The driver/test cleanup did not change the measured candidate or production
implementations. The complete cross-platform wheel matrix was not run locally.
