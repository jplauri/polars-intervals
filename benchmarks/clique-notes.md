# Maximum-weight clique: algorithm and validation notes

The [result report](../docs/clique-benchmarks.md) separates complete Rust calls
from complete release Polars collections. Candidate implementations are private
benchmark/test support; the public expression never selects an algorithm by flag.

## Reduction and correctness

For any finite pairwise-intersecting family of nonempty half-open intervals,
let `a` be the greatest start and `b` the least end. The rows attaining these
extremes intersect (or are the same nonempty row), so `a < b`. Thus every member
contains `a`. Conversely, intervals containing a common coordinate are pairwise
adjacent. Duplicate rows remain distinct vertices.

Removing negative weights improves the objective; removing zero weights preserves
it. The implementation consistently omits both. At a given coordinate every
positive interval containing it can be included. An optimum therefore occurs at
a start coordinate. Departures at `t` must precede arrivals at `t`, and a complete
batch of arrivals is scored before moving on. This computes the exact best
nonempty clique. Empty intervals have no edges and are considered individually;
the final objective is the maximum of that clique, the best positive empty
singleton, and zero.

Only the best coordinate or singleton index is retained. One final pass produces
the original-order mask, without copying an active set on each improvement.
Endpoints need only `Ord + Copy`; no coordinate arithmetic, numeric grid, or
midpoint is used. Checked `i128` additions describe an actual active clique.
Separate cumulative totals of all starts and ends would overflow unnecessarily
on disjoint intervals and are deliberately avoided. Validation finishes before
optimization, including discarded rows and fast paths.

Current ties favor the earliest maximizing coordinate, then a nonempty clique
over an equal-weight empty singleton, then the first original empty row. This
is deterministic across chunks, eager/lazy execution, and explicit/unit defaults;
it is not a cross-release promise about a particular tied mask.

## Candidate costs

- **A, events:** sort two events per positive nonempty row, then sweep with ends
  before starts. Event padding/alignment and requested allocation are measured.
- **B, streams:** sort starts and ends separately and merge with two pointers.
  The packed-record and index-array variants trade sequential access against
  payload size. Unit endpoints need no weight payload. Sortedness checks must
  inspect both relevant streams; sorted starts do not imply sorted ends.
- **C, heap:** sort useful rows by start, then expire ends from a min-heap.
  Sorting costs `O(m log m)` and heap work costs
  `O(m log(max(2, omega)))`, where `omega` is useful concurrency. The heap grows
  on demand; reserving `m` entries would lose its low-concurrency memory benefit.
- **Quadratic:** score candidate starts independently, with separate empty
  singleton handling; only small inputs are timed. No distance-based allocation.

Production retains B's index streams for explicit weights and endpoint-only
streams for implicit units. It folds a common-intersection check into validation
and stops updating the bounds once their intersection becomes empty. Only after
all rows pass validation and a common point is established does it sum useful
weights. This makes full cliques linear without risking false overflow on
disjoint inputs. There is no tiny-input threshold or separate presorted scan.

On the measured 64-bit target, `size_of` includes alignment/padding: endpoints
and indices use 8 bytes, unit or tagged-index events and heap entries use 16,
signed-i128 events use 32, packed endpoint/i64 records use 16, and packed
endpoint/i128 records use 32. The two weighted index arrays therefore request
16 bytes per useful row versus 64 for two i128 packed streams. These are layout
facts; the saved allocator observations separately measure actual peak requests,
allocation counts and heap capacity growth. Scratch is released before output.

All total-call measurements include validation, preparation, allocation,
optimization, and original-order reconstruction. Timing excludes fixtures and
verification. Core batches include output destruction; Python collection timings
exclude it. Untimed allocator measurements include
output and scratch but exclude caller inputs, stack, allocator overhead and RSS.

## Independent checks

Core tests enumerate every subset, including the empty subset, and inspect every
selected pair using the graph definition. A singleton empty row is valid; an
empty paired with anything fails. Tests compare objectives rather than an
arbitrary oracle mask. A separate quadratic start-scoring reference covers larger
random cases. Every benchmark candidate is checked against the subset oracle;
production is invoked directly in benchmarks and verified outside timing.

Proptest uses constructive ordered endpoints and preserves its normal regression
files. The default case count is inherited from proptest; for a longer local run:

```powershell
$env:PROPTEST_CASES = "2048"
cargo test -p intervals-core --locked --test max_weight_clique --test clique_candidates
Remove-Item Env:PROPTEST_CASES
```

Named tests cover exact arithmetic including `i128::MAX`, singleton empties,
nonpositive discarded rows, ties, touching coordinates, duplicate vertices,
ordered nonnumeric endpoints, validation and original row order. Python tests
exercise the real compiled plugin and an independent subset oracle. Large
benchmark masks use linear common-intersection checks rather than pairwise scans.
