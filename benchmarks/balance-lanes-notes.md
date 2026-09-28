# Minimum-lane balancing: implementation and measurement notes

These notes support the [usage contract](../docs/usage.md#balance-lane-row-counts)
and [measured report](../docs/balance-lanes-benchmarks.md).

## Objective and guarantees

For the counts of **all rows**, minimize the integer tuple `(D, Q)`:
`D = max(s) - min(s)` and `Q = sum(s_i²)`. Scores compare spread first;
minimizing squared sizes alone is a different objective. Counts widen to `u128`
before squaring. Since their sum is `n`, the sum of squares is at most `n²`.

The palette size is zero for no rows, otherwise `max(1, omega)`, where `omega`
counts only nonempty concurrency. Empties never conflict, including inside a
nonempty interval; they count toward the balance objective. Only the older
`assign_lanes` promises lane zero for empties. `assign_balanced_lanes` can move
them without adding lanes, whether constructing or improving an assignment.

Properness and minimum palette size are hard guarantees. Balance is heuristic.
Identical endpoints, row order and options are deterministic. Strictly increasing
endpoint transformations preserve comparisons and therefore preserve the result;
arbitrary row permutations need not preserve heuristic quality or labels.

## Construction and repair

The public expression is
`assign_balanced_lanes(start, end, *, initial_lanes=None, max_work=100_000)`.
The default `initial_lanes=None` constructs an assignment; a supplied lane column
or expression starts repair from that coloring. Rust entry points use an optional
initial assignment for the same choice.

With no initial assignment, the old heap assignment remains unchanged and is
always a candidate. Positive-budget construction considers forward and backward
least-loaded sweeps using the global palette from the beginning, stopping early
if a candidate reaches equity. At each nonempty start, release every lane ending
at or before that coordinate. At most `k-1` lanes can be blocked:
otherwise the current interval would witness concurrency above `k`. A free lane
always exists. Choose its smallest assigned count, breaking ties by lane ID.
The backward sweep reverses comparisons; it never negates or subtracts endpoints.
Place empties on least-loaded lanes. Select the smallest `(D,Q)` seed before
repair. This construction never worsens the heap assignment's `(D,Q)` score.
Zero budget returns the original heap assignment after validation.

With `initial_lanes` supplied, repair validates endpoints, label lengths and the
proper, contiguous minimum palette before allocating lane-indexed state. It
rejects gaps, conflicts and nonminimum colorings instead of recoloring them.
It preserves that palette and never worsens the supplied `(D,Q)` score.
Zero budget returns the supplied IDs unchanged after all validation, including
empty and already-balanced inputs.
The incumbent stays valid throughout. For an eligible pair, merge only those
lanes' nonempty start-ordered lists and track the **running maximum end** to
find connected components. A next start at or beyond that end begins a new
component. The immediately preceding end is insufficient for nested stars.
Every empty row is its own singleton, never an interval-graph bridge.

For component side counts `(a_j,b_j)`, put `w_j=abs(a_j-b_j)` and
`B=sum(min(a_j,b_j))`. The attainable pair counts are
`(B+z, B+W-z)`, with `W=sum(w_j)` and `z` a subset sum of the weights.
A compact bitset finds the reachable value nearest `W/2`. First-discovery
predecessors reconstruct a subset with linear storage, without a components-by-W
table. Descending word updates prevent a weight from being used twice.
Zero weights and shifts at machine-word boundaries are explicitly tested.

Accept a recoloring only if the pair's absolute count difference strictly drops.
Its two loads remain within their previous range, so global `D` cannot increase;
their sum is fixed, so `Q` strictly decreases. A move with unchanged global `D`
still helps. Equal-score flips are rejected, precluding cycles. Revisit pairs
after improvements without allocating a quadratic palette table.

For two lanes, a completed exact pair repair finds globally optimal balance.
With three or more lanes, even a complete no-improvement pass is only a pairwise
fixed point. For six disconnected `K2`-joined-leaf gadgets with extra weights
`[9,6,5,5,4,1]`, loads `(15,16,17)` can be pairwise optimal although `(16,16,16)`
is attainable. The test checks this supplied coloring independently of the
multiseed constructor, which is allowed to find equity.

The four stars with leaf counts `[9,8,7,6]` demonstrate why whole-pair subset sum
matters: `(19,15)` needs simultaneous component flips to reach `(17,17)`.
A single-component hill climb gets stuck.

There is no separate dense component-relabeling pass. Forward/backward seeds
already allow disconnected components to use the entire global palette, and
pair repair can exchange their orientations. A component permutation phase would
need its own budget and sparse implementation; it is optional, not a prerequisite
for the guarantees above. No three-color search or general graph/solver framework
is included.

## Resource accounting and stopping

Validation, minimum-color construction and fixed sorting cost `O(n log n)` time
and `O(n+k)` space, independently of the refinement budget. Refinement is **not**
described as `O(n log n)`: subset sum depends on pair sizes and repeated repairs.
Each call shares one deterministic nonnegative `u64` budget. Rust callers pass
it explicitly; Python defaults to 100,000. Python excludes Boolean and serializes
the full unsigned range as decimal text to avoid signed pickle limitations.

Work includes candidate-pair inspection, pair row visits, bitset-word updates,
first discoveries and reconstruction. Exact pair recoloring is atomic: if its
remaining work or checked scratch allocation cannot fit, keep the incumbent.
There is no unlimited all-pairs restart, and no allocation indexed by an
unchecked user label. Stop immediately at the absolute elementary lower bound:
`D=0` if `k` divides `n`, otherwise `D=1`, with empty input handled separately.

The counter reserves conservative work before an operation, rather than counting
CPU instructions. A lane-priority pass costs `k * (floor(log2(k)) + 2)` units;
every candidate pair costs one, even if its sizes differ by at most one. The
bounded merge/component/weight passes for an eligible pair cost its row count
`m` units (a work unit need not equal one physical loop iteration). With `c` pair components,
`h=floor(W/2)` and `b=floor(h/64)+1` words, the subsequent atomic DP/reconstruction
reserves `c*(b+3) + 5*m + 3*h + k + b + 1` units. This covers word processing,
discoveries, initialization, reconstruction and score scans. A rejected reservation
does not consume the remaining budget. Actual wall time also depends on sorting,
allocation and fixed preprocessing; the counter is deliberately reproducible
across machines. There is no separate fixed byte ceiling: sizes are checked and
pair scratch allocations are fallible after their work is authorized. Fixed
preprocessing uses ordinary vector allocation; this is not general OOM recovery.

Doc-hidden core diagnostics distinguish equity, an actually completed pairwise
fixed point, a work limit and a scratch limit. Skips and budget exhaustion are
never labeled local optimality. The public Polars output remains just the lane
Series; end-to-end records mark unavailable diagnostic fields explicitly. `pairs`
counts attempted eligible pairs, including a final skipped attempt; `flips`
counts changed component orientations, not changed rows or successful moves.
`skips` is zero or one because repair stops on the first pair failure. A budget
stop before priority sorting or pair enumeration can have zero skips: read the
stop reason rather than inferring completion from counters.

## Validation evidence

Core deterministic tests and capped proptests check feasibility and minimum `k`
against independent overlap/concurrency checks. Tiny exhaustive colorings break
color symmetry and optimize `(D,Q)` using exactly `k` colors. Separate tiny
pair checks enumerate orientations and validate reconstructed assignments.
Large or capped exhaustive cases remain unknown; no oracle is applied
indiscriminately to the generator's `exact` preset (which includes `n=200`).

Metamorphic checks cover safe translation/scaling, rank compression, row
permutations, empty augmentation and order-only endpoints. Budget-limited repair
is not assumed idempotent. The adapter tests dtype/schema errors independently;
Python integration covers temporal metadata, DST, grouping, slices, unequal
chunk boundaries and streaming whole-group evaluation.

## Corpus and timing scope

The saved measurements predate consolidation into one balancing expression.
Their “repair” method now corresponds to
`assign_balanced_lanes(..., initial_lanes=...)`; “balanced” constructs with the
default `initial_lanes=None`. The algorithms are unchanged. Historical samples,
metadata, source/native hashes and verification records remain untouched.
Rerunning the commands below uses the current interface and records fresh hashes
and timings; it does not reproduce the historical wrapper byte for byte.

The Python consumer reads `iter_dataset(path)` from the existing generator;
IDs and seeds remain in quality records; full parameters, statistics, certificates
and provenance remain linked by original ID in the `.instances.jsonl` sidecar.
The optimizer receives endpoints and, for repair, the
original assignment, never family metadata or a planted target. A valid planted
certificate establishes `D*`; the elementary `delta` alone does not. Missing
optima remain unknown, and no ratio divides by `D*=0`.

Quality is measured once per instance/method/options. Runtime samples aggregate
separately. Every timed result is checked outside timing, and every repair uses
the same original baseline. Core calls include validation, preparation, copying
and returned lane construction; input fixture generation and returned-output
destruction are outside timing. Polars collections additionally include lazy
planning, extraction and output materialization. Full construction includes its
baseline; standalone repair excludes creating the original input coloring.
Repair's internal minimum-palette validation (currently another heap coloring)
is timed; this scope is not a DP-only kernel measurement.
Polars final Series destruction is outside timing, while temporary collection
container teardown can occur inside. Core and Polars timings are
reported separately. Runtime ratios mean candidate median / baseline median,
so values above one mean slower.

Synthetic smoke/exact/scaling datasets and imported JAIST populations retain
their existing format. Full catalogs require explicit download commands; offline
tests use tiny checked-in fixtures. The all and connected populations are kept
separate. Generated datasets live under ignored `target/` or
`benchmarks/generated/`; only measurements and metadata belong in version control.

External integer or physical temporal data can use the same validated path with
the generic `with_empties` family, which permits arbitrary interval geometry:

```python
from generate_interval_graphs import Instance, write_dataset

instance = Instance(
    "with_empties",
    starts,
    ends,
    None,
    {
        "n": len(starts),
        "order": "source",
        "empty_count": sum(s == e for s, e in zip(starts, ends, strict=True)),
    },
    provenance={
        "source": "local",
        "source_file": source_path,
        "source_sha256": source_hash,
        "units": "integer ticks",
    },
)
write_dataset([instance], output_path, kind="external", suite="local", seed=None)
```

The variables are supplied by the caller's data loader. Record source identity,
hash and units; validation checks lengths and integer geometry before writing
shards. Set `PYTHONPATH=benchmarks` when importing from the repository root.
Reversible temporal transforms, groups and chunk/slice variants are runner
workloads, not new corpus families. Consult the runner help for options.

## Reproduce the recorded matrix

The recorded 2026-09-28 run used a completed explicit release install, then
sequential core and Polars measurements without another build or benchmark
running. All three Polars runs have identical source/native hashes. Core has
780 quality/options records and 3,900 raw samples across 60 fixtures; Polars
quality, scaling and execution-variant runs have 1,625, 396 and 1,464
quality/options records with five, five and three timing samples respectively.
All twelve representation/engine variants agree in exact output lane hashes.

### Measured default and ablation

With 100,000 units, constructor spread improves on 29/60 core fixtures, with
50 equity stops, four completed pairwise fixed points and six budget stops.
Baseline repair improves spread on 16/60, with 31 equity stops, four fixed points
and 25 budget stops. No recorded work counter exceeds its option's budget.
Compared with 10,000, the default improves constructor score on four fixtures and
repair on sixteen. One million units further improves two constructor cases and
fifteen repairs. The default limits extra search; it is not a saturation claim.

Sorted core `long_short`, 10,000 rows, illustrates that choice:

| Work budget | Constructor D | Constructor median ms | Repair median ms |
| ---: | ---: | ---: | ---: |
| 10,000 | 9,499 | 0.7943 | 0.2208 |
| 100,000 | 4,728 | 0.8694 | 0.2927 |
| 1,000,000 | 7 | 1.4189 | 0.8948 |

The benchmark-only multiseed experiment divides one budget between baseline,
forward and backward repair, reusing production functions. Repeated validation
and preparation are included. It never beats single-best-seed polishing in this
matrix, and loses two, three and two fixtures at the three budgets. At the default
its median matched-instance runtime ratio versus the constructor is 4.40×.
This is the repair-stage ablation supporting one polished seed. Individual
forward/backward candidates lose to baseline on 14/60 and 6/60 fixtures; those
losses remain in the saved records. A separate component-permutation stage was
not implemented or measured, so no empirical benefit or cost is claimed for it.

Maximum requested-live-heap peaks across the default core fixtures are 7,545,728
bytes for construction and 6,597,504 for repair, measured outside timing. These
include output, exclude caller inputs and allocator overhead, and are not RSS.

### Distinct corpus quality

Each cell below is **D-improved / Q-only improved / unchanged** against that
instance's original baseline. Corpora stay separate, including overlapping IDs:

| Corpus | Constructor | Repair |
| --- | ---: | ---: |
| Smoke seed 42 (30) | 14 / 2 / 14 | 14 / 2 / 14 |
| Smoke seed 43 (30) | 12 / 0 / 18 | 12 / 0 / 18 |
| Exact preset (450) | 277 / 0 / 173 | 276 / 1 / 173 |
| Scaling preset (132) | 108 / 0 / 24 | 68 / 8 / 56 |
| Handcrafted (20) | 6 / 0 / 14 | 4 / 0 / 16 |

The seven all-catalog and four connected-catalog fixture records are unchanged
and optimal. On exact's 72 certified cases, baseline hits 28 optima and both
balancing modes hit all 72; the other 378 optima remain unknown. On scaling's 24
certified cases, baseline/repair/construction hit 0/12/24. The worst repaired certified gap
is 711 on sorted planted `k=8`, 100,000 rows, instance
`353fc1f57b0d43aa4e98675d8573f5c6f4072d4e7c01fe9cbe9f37cbc85b7563`:
baseline/repair/construction take 1.8037/3.7360/6.3477 ms, with D=711/711/0.
The supplied simultaneous-flip and pairwise-limitation assignments are separate
`repair_supplied` records and are not mixed into baseline-repair counts.

### Commands

Generate the existing suites and import the two **offline fixture** populations
before measuring. These commands create ignored data, not a new corpus format:

```powershell
python benchmarks/generate_interval_graphs.py synthetic --suite smoke --seed 42 --output target/balance-work/smoke
python benchmarks/generate_interval_graphs.py synthetic --suite smoke --seed 43 --output target/balance-work/smoke-43
python benchmarks/generate_interval_graphs.py synthetic --suite exact --seed 42 --output target/balance-work/exact
python benchmarks/generate_interval_graphs.py synthetic --suite scaling --seed 42 --output target/balance-work/scaling
python benchmarks/generate_interval_graphs.py jaist --input benchmarks/fixtures/jaist_interval_list_tiny.txt --catalog-kind all --output target/balance-work/jaist-all
python benchmarks/generate_interval_graphs.py jaist --input benchmarks/fixtures/jaist_interval_connected_tiny.txt --catalog-kind connected --output target/balance-work/jaist-connected
```

The following uses the shared release-plugin setup. Choose fresh output prefixes
when repeating a run; neither generator nor runner silently overwrites results.
Run sequentially after compilation and tests finish:

```powershell
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
$balancePreviousThreads = $env:POLARS_MAX_THREADS
$env:POLARS_MAX_THREADS = "1"
uv run --no-sync python benchmarks/balance_lanes.py --dataset target/balance-work/smoke --dataset target/balance-work/smoke-43 --dataset target/balance-work/exact --dataset target/balance-work/jaist-all --dataset target/balance-work/jaist-connected --handcrafted --stress-sizes 1000 10000 100000 --max-work 100000 --warmups 1 --repeats 5 --output benchmarks/results/balance-polars-quality-windows-20260928
uv run --no-sync python benchmarks/balance_lanes.py --dataset target/balance-work/scaling --max-work 100000 --warmups 1 --repeats 5 --output benchmarks/results/balance-polars-scaling-windows-20260928
uv run --no-sync python benchmarks/balance_lanes.py --dataset target/balance-work/smoke --handcrafted --stress-sizes 1000 --max-work 100000 --workloads integer date datetime grouped sliced multi_chunk --engines auto streaming --warmups 1 --repeats 3 --output benchmarks/results/balance-polars-workloads-windows-20260928
$env:POLARS_MAX_THREADS = $balancePreviousThreads

$env:BALANCE_SIZES = "1000,10000,100000"
$env:BALANCE_BUDGETS = "10000,100000,1000000"
$env:BALANCE_SAMPLES = "5"
$env:BALANCE_CSV = Join-Path (Get-Location) "benchmarks/results/balance-core-windows-20260928.timings.csv"
$env:BALANCE_QUALITY_CSV = Join-Path (Get-Location) "benchmarks/results/balance-core-windows-20260928.quality.csv"
cargo bench -p intervals-core --bench balance_lanes --locked
```

The core output paths above are absolute because Cargo starts a benchmark in its
crate directory. The harness refuses to overwrite existing files.

The report's core table uses `balance-core-windows-20260928.default-timings.csv`,
an exact subset of the full raw matrix with `budget` in `{0,100000}`. Zero belongs
to baseline/seed-only scopes; 100,000 belongs to repair/construction scopes.
This keeps one option per method without averaging budgets. Recreate it using:

```python
import polars as pl

prefix = "benchmarks/results/balance-core-windows-20260928"
(
    pl.scan_csv(prefix + ".timings.csv")
    .filter(pl.col("budget").is_in([0, 100000]))
    .collect()
    .write_csv(prefix + ".default-timings.csv")
)
```

All 2,100 selected samples remain unmodified; the metadata records hashes of
both full and derived CSVs. Run the shared reporting generator to recreate the
Markdown tables and their median/range downloads.

The all/connected catalog fixtures have seven and four records respectively;
they are not full JAIST catalog measurements. Smoke contains 30 records per seed,
exact 450, and scaling 132. IDs can recur across suites: quality summaries keep
their corpus dimension, and do not pool overlapping smoke/exact populations.
The workload run repeats 40 logical instances in six representations and two
engines. These are equivalence/timing variants, not 480 independent geometries.
Full catalog downloads remain explicit commands in the
[generator documentation](README.md#exhaustive-jaist-import).

Core microfixtures use the existing Rust benchmark support and are separate from
the corpus-driven Polars workloads. Family names do not identify interchangeable
geometries across those scopes: core `long_short` inserts a long interval every
32 rows, while the handwritten Polars `long_short` is one unavoidable star.
Core `nearly_clique` adds empty rows after a clique; the handwritten Polars case
uses a clique of long intervals and disjoint short leaves. Input definitions and
source hashes, not family names alone, identify the measured instances.
