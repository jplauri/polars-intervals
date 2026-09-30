# Interval geometry comparison notes

The [report](../docs/interval-geometry-benchmarks.md) contains measured results.
The [API guide](../docs/interval-geometry.md) defines the public contracts.

Saved timings and RSS measurements describe the archived implementations.
Later cleanup shared the clustering boundary predicate, adapter domain/type
conversions, validation barrier and benchmark provenance helpers. It removed
redundant adapter checks and replaced the competitor's benchmark-only Rust
validator with Polars checks. The public Python APIs and geometry algorithms
retain their contracts. Raw samples, metadata and source archives are unchanged.
The cleanup was checked for behavior, without new timing or memory claims.

## Candidates and timing boundaries

The core runner calls the actual public core functions as `production`.
The private candidates deliberately vary record layout, sortedness detection
and gap emission. They share endpoint validation, but have independent scan
implementations covered by the same original-problem oracles.

| Method | Preparation and scan |
| --- | --- |
| `production` | Scan original endpoints directly after verifying relevant start order, otherwise sort packed records. Clustering restores canonical IDs. Gaps emit directly. |
| `packed_sort` | Packed records with unconditional sorting. Measures the value and cost of checking start order. |
| `packed_sorted` | Private packed preparation with verified start order. Compares with the production direct-buffer path and unconditional packed sorting. Its fused gaps also compare directly with `materialized`. |
| `indices_sorted` | Sort row indices and read endpoints indirectly, with a verified start-order fast path. Includes index storage and complete output construction. |
| `materialized` | Build canonical clipped union, then complement it. Checks start order. Gaps only. |

The archived runs also include `indices` and `materialized_sort`, which always
sort. These private variants were removed after measurement. The retained
packed pair isolates the sortedness check, and `packed_sorted` versus
`materialized` isolates gap emission. `indices_sorted` retains the indexed
comparison and its measured wins and memory savings.

For verified ordered input, the only allocation is output. Ordered clustering
assigns each new component its canonical ID while visiting original rows,
including isolated empties. Union and gaps emit directly from endpoint buffers.
Gap ordering is checked after clipping, so discarded outside-domain rows cannot
disable this path. Every row is still validated before that check.

The initial packed production and the later private `borrowed_sorted` prototype
have separate saved runs and source archives. The prototype was promoted after
repeat measurements showed substantial ordered-input runtime and memory gains.
Its duplicate private implementation was then removed. Those earlier timings
are historical evidence for the decision, not measurements of the final code.

- Initial packed route: [samples](results/interval-geometry-core-20260929.csv),
  [metadata](results/interval-geometry-core-20260929.metadata.json),
  [source snapshot](results/interval-geometry-core-20260929.sources.zip).
- Direct-buffer prototype: [samples](results/interval-geometry-core-borrowed-20260929.csv),
  [metadata](results/interval-geometry-core-borrowed-20260929.metadata.json),
  [source snapshot](results/interval-geometry-core-borrowed-20260929.sources.zip).

The native Polars competitor sorts nonempty rows and computes a grouped prefix
maximum of ends. Shifting that maximum supplies the frontier before each row.
Separate expression stages mark boundaries and number runs. Union aggregates
run extrema. Clustering attaches the first original row of each component,
adds empty rows as distinct components, restores row order and densely ranks
component representatives within each group. Gaps derive leading, internal
and trailing uncovered ranges from clipped union. Observed groups are obtained
before clipping, with nullable keys mapped to stable first-row positions.

The measured native competitor used the package's Rust endpoint/domain
validator. The current competitor checks original rows with Polars expressions
and validates scalar bounds with the existing native-baseline helper. A direct
eager validation call precedes the scan plan. For lazy inputs, a blocking node
prevents downstream filters, projections or slices from changing the validation
instance. All geometry after that barrier uses native Polars expressions.
Temporary columns occupy a fresh projected namespace. No callback collects a
lazy query or loops over rows/groups in Python. After a grouped clustering
validation failure, one native eager window supplies the original within-group
position directly, without parsing an exception message. Valid calls do not run
this error-only calculation.

The clustering competitor takes a frame and column names, equivalent to a
selection of the public expression. It is not an alternative arbitrary-expression
API. Its grouping supports Polars window key dtypes and endpoint-named keys.
The merge/gap competitors enforce the public frame grouping contract.
All three competitors return lazy plans when given lazy input.

Rust samples include complete calls and destruction of returned vectors.
Polars eager and complete-lazy samples include argument checks, input
preparation, validation, planning, grouping, sorting, scanning, restoration
and output construction. Returned Python output destruction is excluded.
Separate plan-only samples never execute the query. Prebuilt-plan collection
samples exclude plan construction. Scan-backed collection includes Parquet
I/O and is identified separately.

## Correctness arguments

After sorting nonempty rows by start, let the frontier be the maximum end
seen in the current component. A row starting before that frontier overlaps
a preceding row attaining it. A row starting at or after it cannot strictly
overlap any preceding component row. Replacing the strict boundary with a
strictly larger start gives touching-inclusive connectivity. Empty rows are
excluded from this reasoning and remain independent singletons. Mapping each
component to its first original row yields the required deterministic IDs.

For union, a start at or before the current frontier leaves no positive gap,
so extending the frontier preserves exactly the covered set. A larger start
creates a positive gap and therefore forces a new maximal run. The final
runs are sorted, nonempty and strictly separated.

Clipping can only replace a boundary with a domain boundary. A cursor over
the clipped runs identifies every uncovered range between the last covered
end and the next start. Leading and trailing ranges complete the domain
complement. Empty/clipped-away rows cannot split gaps. Validation precedes
every clipping or empty-domain shortcut.

Small tests use direct pairwise graph traversal for clustering and direct
membership on elementary boundary cells for union/gaps. These oracles do not
sort and scan with the production frontier reduction. Bounded integer bitmap
tests provide another independent check. Larger benchmark instances compare
entire canonical outputs and structural invariants. They do not have an
independent large-instance oracle.

## Memory and omissions

Core memory is peak requested live heap in a separate untimed allocator call.
It includes temporary buffers, output and reallocations. It excludes caller
inputs, allocator overhead, stack and process RSS. Indices avoid copied endpoint
pairs but still allocate indices and outputs.

Optional Polars memory workers start in fresh processes. They record working
set/RSS and process high-water marks before and after one complete eager call,
retaining its output. These include inputs and runtime. The increase over the
post-fixture high-water mark is not an allocation count and can be zero when
fixture construction previously consumed more memory. Memory collection does
not run inside timed samples.

The coverage-profile baseline is optional and omitted. No profile is needed
to compute these geometry outputs. The native prefix-max competitor already
provides a different complete execution route. No bulk-array Rust variant is
retained without evidence that its extra intermediate arrays improve the
direct scan. Narrow core cases are omitted when a coordinate does not fit.
The reports identify measured thread counts, sizes, repeats and omissions.
