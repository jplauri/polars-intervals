# Coverage profile correctness and implementation notes

These notes support the [coverage profile report](../docs/coverage-profile-benchmarks.md).
Measured comparisons and production selection belong in that report. The
implementation discussed here is the retained two-stream Rust sweep.

## The function being constructed

For half-open intervals `[s_i, e_i)` with nonnegative quantities `q_i`, the
profile is `L(t) = sum(q_i where s_i <= t < e_i)`. Unit mode uses `q_i = 1`.
The result consists of maximal, nonempty, constant-load segments. Sparse mode
omits zero segments without joining positive segments across the omitted gaps.
This operation constructs a function over coordinates and does not select rows.

An explicit domain clips the represented function. Without bounds, the domain
is the hull of every nonempty input interval, including zero-quantity rows.
Empty rows never establish a domain. These rules also determine the zero tails
when full mode is requested. A grouped empty table has no observed groups.
An ungrouped empty table still has one collection, so explicit nonempty bounds
can produce one zero segment.

## Validation and exact arithmetic

The core checks lengths, reversed bounds, every interval, and every quantity
before returning from an empty-domain or no-contribution case. Invalid interval
and negative-load diagnostics refer to original slice positions. The Polars
adapter validates these rows globally before gathering groups, preserving eager
DataFrame indices. No group result is returned until all groups have succeeded.

Only positive clipped nonempty intervals contribute events. Consequently,
overlapping quantities outside the requested domain cannot overflow its result.
Zero-weight rows contribute domain bounds but need no load events. Empty rows
need neither events nor bounds, regardless of their quantity.

Loads use checked `i128` additions and subtractions. The algorithm never computes
total input quantity, cumulative arrivals independently of departures, or
duration times load. Those quantities can overflow even when the profile itself
is representable. Generic core quantities convert losslessly to `i128`, including
`i128` test inputs. Public Polars weights are restricted to signed and unsigned
8/16/32/64-bit integers. Public loads are always non-null `Int128`.

## Sweep invariant and canonical output

After clipping, every arrival is inside the domain and strictly precedes its
paired departure. Starts and ends are sorted independently. Before processing
the next coordinate `t`, the maintained load is exactly the sum of quantities
active on `[previous, t)`. There are no contributing endpoints inside that cell,
so emitting its load represents the function everywhere on the cell.

At `t`, all departures are subtracted before any arrivals are added. Processing
the whole batch establishes the same invariant for the next cell. Every addition
in that batch increases the load toward the next cell's actual load. Thus an
overflow during arrivals proves that the next positive-length represented cell
is unrepresentable; there is no temporary overlap of touching intervals. The
right domain boundary is emitted up to, but never processed as a new cell.
The sweep also advances through departures after the final arrival.

Emission merges a cell with the previous segment only when their loads agree
and the previous segment ends exactly at the cell's start. The equality test
removes boundaries where contributing row identities change but the load does
not. The touching test prevents merging across a positive-length omitted gap.
After the final domain edge, all represented cells have been emitted or suppressed
according to the zero option. This gives the unique canonical profile independent
of input order, chunk layout, or how event batches were discovered.

No endpoint subtraction, midpoint, predecessor, successor, or coordinate-range
allocation occurs in production. The same argument applies to ordered endpoints
such as characters as well as integers and temporal physical values.

## Complexity and layout

Let `n` be the input row count, `m` the number of positive clipped nonempty rows,
`u` the number of contributing distinct endpoint coordinates plus domain edges,
and `z` the final segment count. Validation and domain inference cost `O(n)`.
Preparing the two streams costs `O(n)` and sorting costs `O(m log m)`.
The merge processes every contributing endpoint once and emits `z` segments,
so complete core time is `O(n + m log m + z)`, with `z <= u - 1` for a nonempty
domain. The extra storage is `O(m + z)`. A numeric coordinate span does not enter
either bound.

Each stream is checked for sortedness separately. Sorted starts alone do not
imply sorted ends. If both streams are already ordered, checks and the sweep
take `O(n + z)` total time, including validation and clipping. Clipped coordinates
are stored in each stream and both sortedness checks are included in timing.

Unit mode stores two endpoint-only vectors and generates unit deltas during the
merge. It allocates no ones vector. Weighted mode stores two contiguous vectors
of `(coordinate, quantity)` records. Keeping the input quantity type avoids
widening the records unnecessarily. Natural size and alignment, rather than
unsafe packed layouts, determine their allocations. With 64-bit endpoints,
the measured target uses 16-byte records for 64-bit quantities and 32-byte records
for `i128` quantities. The native adapter supplies `i128` extracted quantities,
so its two weighted stream buffers occupy `64m` bytes before output.

The private index-stream comparison preserves the earlier implementation. Its
two index buffers occupy `2 * m * sizeof(usize) = 16m` bytes on the measured
64-bit target, but sorting and scanning must read original endpoint/quantity
buffers through those indices. The record layout trades this substantial space
advantage for contiguous access and stronger measured performance on large
shuffled inputs. Ordered and heavily coalesced cases retain meaningful costs
and losses, which are shown in the report. The private event candidate uses
`(coordinate, tagged_index)` records. See raw layout fields and metadata for
concrete allocation measurements and the exact measured implementation.

The active-end heap candidate additionally holds live intervals, which can be
bounded by concurrency, but its start-order preparation and output still occupy
memory. It does not reserve `n` heap slots. Allocator measurements include the
complete candidate's live requested heap, including output; they are neither
whole-process RSS nor just active-state capacity.

The Polars adapter adds endpoint/weight extraction, native stable grouping,
gathering, key repetition, and output construction. It preserves all chunks as
one instance per key tuple. These costs are included only in complete Polars
measurements, separately from the core timings. Eager inputs execute immediately;
lazy inputs defer the same native call until query execution. Native work releases
the GIL in both routes.

The lazy route supplies an exact output schema to a non-streamable Polars
`map_batches` node. It selects required input columns by literal name, then
blocks downstream predicate, projection and slice pushdown across the profile.
The callback hands one complete frame to the native binding; it performs no
Python row or group calculation. This barrier remains whole-collection under
the streaming engine, so the native proof and chunk/group semantics apply
unchanged. Surrounding query stages can still optimize and stream.

Schema resolution does not compute the profile. Argument/schema checks happen
during construction, while native bounds and row validation are deferred. A
query whose result makes the profile unnecessary can eliminate the node entirely
(for example `head(0)`), in which case its row validation does not run. Every
executed profile still validates all rows before clipping, even for an empty
domain. Tests cover filtering, selecting output columns, taking a nonzero head,
partitioned scans, mutable option snapshots, and both collection engines.

## Independent checks

The main small-instance oracle establishes its domain from original rows,
collects distinct boundaries, and scans all original rows at each elementary
cell's left endpoint using `s_i <= t < e_i`. It independently coalesces the
result and does not reuse production validation, clipping, or sweep helpers.
A second small integer oracle scans each tick without constructing breakpoints.
Python's selected extreme examples use arbitrary-precision integer sums.

Rust named tests cover domain and zero semantics, duplicates and empty rows,
equal-time batches, gaps, nesting and tail expiration, all input orderings,
generic ordered endpoints, validation diagnostics, and exact overflow behavior.
Production Proptest checks include complete oracle equality, structure, nonlinear
order-preserving relabeling, translation, quantity scaling, duplication, splitting,
empty/zero additions, independent subset addition, idempotence, clipping, and
sparse/full equivalence. Bounded tests check area conservation only as an extra
invariant. Profile peaks are cross-checked against clique weight and unit lane
count after empty rows and clipped-empty rows are removed.

The private benchmark candidates also compare their entire canonical outputs
with the independent membership oracle and per-tick checks. Candidate agreement
on a large fixture is supplementary evidence, not an independent proof of its
load function. Benchmark harnesses avoid applying a quadratic oracle to
million-row inputs.

The tests use Proptest's default configuration and honor `PROPTEST_CASES`.
A higher-case PowerShell run is:

```powershell
$env:PROPTEST_CASES = "2048"
cargo test -p intervals-core --test coverage_profile --test coverage_profile_candidates --locked
Remove-Item Env:PROPTEST_CASES
```

Source-parallel Proptest persistence retains any minimized regressions. No
artificial regression seed is added when the generated cases uncover no failure.
