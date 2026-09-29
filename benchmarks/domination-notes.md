# Dominating-set algorithms and verification

## Reduction and exactness

Validate every endpoint/cost first. Separate empty rows: each is an isolated
vertex forced into every feasible set. Among nonempty rows, retain one copy
of each inclusion-minimal geometry as a target, but keep **every original
nonempty row as a candidate**. Sorting by decreasing start, increasing end,
then row and retaining each strict new minimum end extracts targets in
`O(n log n)` time. Reverse the retained list. Target starts and ends are
strictly increasing; targets can overlap and are never merged.

Every original nonempty row contains a minimal target (follow strict
containment in the finite input). Any interval overlapping that target also
overlaps its containing row. Thus a subset dominates the original graph iff
it selects every empty row and overlaps every minimal target. A candidate
`[s,e)` overlaps precisely targets `[a,b)`, where `a = count(target.end <= s)`
and `b = count(target.start < e)`. Both inequalities respect touching endpoints;
`a < b` because the candidate contains a minimal target. Covering all integer
unit segments of `[0,m)` is the same condition, preserving every subset's
feasibility, cost and cardinality after forced empties. No endpoint subtraction,
epsilon, coordinate predecessor, or elapsed-distance allocation is used.

## Candidates

- **Reduction (A):** materialize auxiliary endpoints in original row order,
  call existing `minimum_cover`/`minimum_cost_cover`, restore empties and check
  the final cost. This is the complete reuse baseline.
- **Fused covering (A):** reuse extracted start order for unit covering;
  for weighted covering, sort blocks by end and call the existing covering
  Fenwick DP/reconstruction directly. Avoid auxiliary arrays, repeat input
  validation/clipping, and a copied candidate list. Covering itself is unchanged.
- **Heap prefix DP (B):** reuse the reduction's start order; at target `j`
  insert each block beginning at `j` with proposal `F[j]+(cost,1)`, original
  row, predecessor `j`, and expiration `b`. Remove roots with `b <= j`, then
  the best remaining proposal defines `F[j+1]`. The heap can retain buried
  expired proposals, requiring `O(n)` storage even if few intervals are active.
- **Direct unit greedy (C):** scan demands by finish and candidates by start.
  For the earliest-ending undominated demand, select an intersecting candidate
  with the greatest end, then continue beyond that domination frontier. Keep
  already-dominated rows in the candidate prefix maximum. No overlap pairs or
  repeated full scans are materialized. Common explicit costs, including zero,
  have the same cardinality optimum.
- **Quadratic prefix DP:** the same recurrence with a full block scan at each
  target, bounded to tiny benchmark instances. It helps isolate heap bugs but
  is not an independent correctness oracle.

For B, let `F[j]` optimize coverage of targets before `j`; it may cover extra
targets. Adding `[a,b)` containing `j` to `F[a]` covers through `j`, an upper
bound. Conversely choose a row covering `j` in an optimal prefix set and
remove it: every target before `a` remains covered, so its remainder's
objective is at least `F[a]`. This proves
`F[j+1] = min_lex(F[a_i]+(c_i,1): a_i <= j < b_i)`.
An optimal prefix never needs a candidate starting at or after its length:
nonnegative cost and secondary cardinality remove such an irrelevant row.
Therefore `F[a]` has not already selected the row being added. Backpointers
strictly decrease (`a <= j < j+1`), so reconstruction cannot charge a row twice.

For C, let `d` be the earliest-ending remaining demand and `x` a row in an
optimal completion dominating it. The greedy `g` intersects `d` and ends at
least as late as `x`. Every remaining demand `u` ends at least at `d.end`,
which exceeds `g.start`. If `u` overlaps `x`, its start is less than `x.end`,
hence less than or equal to `g.end` with a strict first inequality. Thus `g`
also dominates `u`. Replacing `x` with `g` cannot increase count. Remaining
undominated demands start at or after `g.end`, establishing the sweep frontier.
This exchange proof does not apply to heterogeneous costs.

All practical candidates take `O(n log n)` time and `O(n)` space end to end.
All checked additions discard unrepresentable alternatives; `None`, not
`i128::MAX`, denotes unreachable. Because costs are nonnegative, discarded
paths cannot return to representable range. Forced empty costs are included
in the checked selected optimum, without summing all input costs as a bound.

## Independent checks

Rust subset enumeration checks the original adjacency definition with explicit
self-domination and isolated empties. It calls no production reduction or
feasibility helper. Every candidate is checked against its optimum pair.
Separately, all subsets of small instances check reduction equivalence;
pairwise strict containment verifies target extraction; direct target overlap
verifies every block. Metamorphic proptests cover units/zeros, permutations,
translations/relabeling, scaling/monotonic costs, empty addition, disconnected
unions, cloning, enlargement and private witnesses. Generic ordered endpoints
and `i128` overflow cases have direct tests. Higher-case command:

```powershell
$env:PROPTEST_CASES = '2048'
cargo test -p intervals-core --locked --test minimum_cost_dominating_set
```

Python tests exercise the compiled plugin with an independent subset oracle,
logical dtypes, physical precision, schema errors, eager/lazy/group contexts,
chunks and streaming. Scalar costs and unsupported wider dtypes are rejected.

Core benchmark verification independently sorts selected nonempty intervals
by start and queries prefix maximum ends for each unselected demand. Every
empty is checked separately. Subset enumeration supplies exact optima for
`n <= 12`; isolated rows, full cliques/nesting, stars, and uniform paths have
analytic optima. Other rows explicitly say `candidate_agreement`, alongside
feasibility. Python benchmarks label their large nonanalytic weighted cases
`feasibility_only`, without claiming optimality from that check.

Timing covers full core calls or prepared Polars collections, including
validation, preprocessing, reconstruction and mask creation. Input generation,
checking and returned-output destruction are excluded; cleanup of internal
temporary buffers is timed. Candidates rotate per
sample after warmups. Separate allocator calls measure peak requested live
heap and allocation counts, including internal buffers/output and excluding
caller inputs, allocator overhead, stack and RSS. This is not buffer capacity
or process RSS. All original nonempty candidates remain; CSV `candidates`
reports that count, and `m` reports distinct minimal targets.

The pilot and large comparison were recorded before production selection;
their `production` label means the original reduction-plus-cover baseline.
The `production` and `final-large` result files measure the final public entry
points (heap for heterogeneous costs; greedy for units and uniform costs).
Do not pool these runs. The later production matrix also adds duplicate-heavy
geometry with costs of 1 versus 1,000,000.

## Measurement sources and cleanup

The saved timings precede the repository-wide cleanup merged in
[`edf743b`](https://github.com/jplauri/polars-intervals/commit/edf743b).
The feature snapshot at
[`510cd44`](https://github.com/jplauri/polars-intervals/commit/510cd44)
preserves the final measured implementation and runners. Historical metadata
records the base revision plus per-file source hashes because the feature was
uncommitted during measurement. Raw samples and their metadata remain unchanged.

The subsequent Ponytail pass reuses shared length validation, Python plugin
registration, random/shuffle functions and benchmark provenance. It removes an
unused target-count helper and a greedy forwarding wrapper, and moves shared
endpoint checks into the existing test tables. It also adopts the current Rust
length-error representation. The greedy, target reduction and heap recurrence
are unchanged. The published numbers describe the earlier measured build,
not a new timing run after these helper changes. Post-cleanup correctness and
build checks are recorded in
[the verification record](results/domination-verification-windows-20260929.md).

No literature challenger, candidate-block coalescing or parallel optimizer was
implemented or measured. The benchmark-only covering alternatives remain here
to support the requested comparisons and independent candidate checks.
