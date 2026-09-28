# Benchmarks

See the [benchmark overview](../docs/benchmarks.md) for compact results tables.

[Running and publishing benchmarks](../docs/benchmarking.md) covers setup,
table/plot generation, and the four-section checklist for adding an operation.

## Python scripts

Run commands from the repository root. Benchmark runners need an installed
release plugin. Summary and plotting scripts only read saved results.

| Script | Role | Input → output | Report |
| --- | --- | --- | --- |
| [`generate_interval_graphs.py`](generate_interval_graphs.py) | Offline corpus generator | Synthetic presets or exhaustive JAIST catalogs → validated, sharded interval instances | [Interval-graph corpora](#interval-graph-corpora) |
| [`test_generate_interval_graphs.py`](test_generate_interval_graphs.py) | Corpus checks | Tiny deterministic instances and local fixtures → structural and reproducibility checks | [Interval-graph corpora](#interval-graph-corpora) |
| [`nesting_depth.py`](nesting_depth.py) | End-to-end benchmark | Exact analytic families, integer/temporal and grouped inputs → verified collection samples and environment metadata | [Nesting depth](../docs/nesting-depth-benchmarks.md) |
| [`balance_lanes.py`](balance_lanes.py) | End-to-end benchmark | Existing interval corpora → independently checked row-count quality, collection samples and provenance | [Lane balancing](../docs/balance-lanes-benchmarks.md) |
| [`test_balance_lanes.py`](test_balance_lanes.py) | Runner checks | Tiny offline records → oracle, scoring, baseline alignment and aggregation checks | [Lane balancing notes](balance-lanes-notes.md) |
| [`coverage_summary.py`](coverage_summary.py) | Report generator | Core candidate CSV → median runtime and allocation CSV | [Maximum k-coverage](../docs/coverage-benchmarks.md) |
| [`coverage_temporal.py`](coverage_temporal.py) | End-to-end benchmark | Integer/temporal fixtures → raw timing CSV | [Maximum k-coverage](../docs/coverage-benchmarks.md) |
| [`overlap_count.py`](overlap_count.py) | End-to-end benchmark | Generated workloads → timing samples, RSS and environment in JSON | [Overlap counting](../docs/overlap-count-benchmarks.md) |
| [`containment_count.py`](containment_count.py) | End-to-end benchmark | Generated workloads → samples, native plans and pair-count safety skips in JSON | [Containment counting](../docs/containment-benchmarks.md) |
| [`weighted_summary.py`](weighted_summary.py) | Report generator | Candidate CSV argument → runtime ratios and tables on stdout | [Weighted scheduling](../docs/weighted-scheduling-benchmarks.md) |
| [`capacity_summary.py`](capacity_summary.py) | Report generator | Candidate CSV arguments (default: recorded run) → median timing/allocation CSV on stdout | [Capacity scheduling](../docs/capacity-scheduling-benchmarks.md) |
| [`capacity_temporal.py`](capacity_temporal.py) | End-to-end benchmark | Integer/temporal cliques → raw timing CSV on stdout | [Capacity scheduling](../docs/capacity-scheduling-benchmarks.md) |
| [`capacity_profile_summary.py`](capacity_profile_summary.py) | Report generator | Profile candidate CSVs → median runtime, flow and allocation metrics | [Capacity profiles](../docs/capacity-profile-benchmarks.md) |
| [`capacity_profile_temporal.py`](capacity_profile_temporal.py) | End-to-end benchmark | Integer/temporal profiles → verified release-native timing CSV | [Capacity profiles](../docs/capacity-profile-benchmarks.md) |
| [`covering_summary.py`](covering_summary.py) | Report generator | Candidate CSV argument (default: recorded run) → ratios and tables on stdout | [Covering](../docs/covering-benchmarks.md), [cost covering](../docs/cost-covering-benchmarks.md) |
| [`covering_temporal.py`](covering_temporal.py) | End-to-end benchmark | Integer/temporal covering fixtures → raw timing CSV on stdout | [Covering](../docs/covering-benchmarks.md), [cost covering](../docs/cost-covering-benchmarks.md) |
| [`stabbing_summary.py`](stabbing_summary.py) | Report generator | Recorded core and optional temporal CSVs in `results/` → median/ratio tables on stdout | [Stabbing points](../docs/stabbing-benchmarks.md) |
| [`stabbing_temporal.py`](stabbing_temporal.py) | End-to-end benchmark | Integer/temporal fixtures → raw timing CSV on stdout | [Stabbing points](../docs/stabbing-benchmarks.md) |
| [`plot.py`](plot.py) | Shared report generator | [`plots.toml`](plots.toml) and saved CSV/JSON → compact Markdown tables, exact summary CSV, optional SVG | [Reporting workflow](../docs/benchmarking.md#generate-plots) |
| [`test_plot.py`](test_plot.py) | Reporting checks | Small synthetic records → checks for units, repeats, invalid input and missing cases | [Plot workflow](../docs/benchmarking.md#generate-plots) |

Rust candidate runners live in [`crates/intervals-core/benches/`](../crates/intervals-core/benches/).

The lane-balancing core runner uses `cargo bench -p intervals-core --bench
balance_lanes --locked`. Set `BALANCE_CSV` and `BALANCE_QUALITY_CSV` to new
absolute output paths (Cargo runs inside the crate directory);
`BALANCE_SIZES`, `BALANCE_BUDGETS`, and `BALANCE_FAMILIES` accept
comma-separated lists, and `BALANCE_SAMPLES` selects repeated timing samples.
It compares the existing baseline, forward/backward/best seeds, baseline repair,
the public balanced constructor, and a benchmark-only shared-budget multiseed
experiment. The public constructor and repair now share `assign_balanced_lanes`:
omit `initial_lanes` to construct, or supply it to repair. Saved measurements
predate that interface consolidation and preserve their original metadata.
Quality and untimed requested-heap measurements are separate from
raw timings. See [the balancing notes](balance-lanes-notes.md) for scopes and
the [report](../docs/balance-lanes-benchmarks.md) for measured settings.
They do not require Python or the Polars plugin. Each report names its Cargo
target. Covering and cost covering share the `covering` target.

## Interval-graph corpora

`generate_interval_graphs.py` prepares inputs for future coloring and balancing
experiments. It uses only the Python standard library and runs without installing
the package or compiling Rust. It performs no coloring, optimization, or algorithm
timing. Synthetic families explore controlled geometry and larger sizes; the
[JAIST catalog](http://www.jaist.ac.jp/~uehara/graphs/#interval) supplies exhaustive
non-isomorphic small interval graphs.

```sh
python benchmarks/generate_interval_graphs.py list-families
python benchmarks/generate_interval_graphs.py synthetic --suite smoke --seed 42 --output benchmarks/generated/interval_graphs
python benchmarks/generate_interval_graphs.py synthetic --suite exact --seed 42 --output benchmarks/generated/interval_graphs-exact
python benchmarks/generate_interval_graphs.py synthetic --suite scaling --seed 42 --output benchmarks/generated/interval_graphs-scaling
```

`smoke` is the default and quickly exercises every family. `exact` uses several
sizes in the 20–200 range, replicates, and density regimes for later solver
comparisons. `scaling` uses 1,000, 10,000, and 100,000 intervals, including several
planted clique sizes. Presets are ordinary Python data in the generator.

| Families | Geometry |
| --- | --- |
| `random_endpoints`, `start_duration`, `fixed_duration` | Ordered random endpoints, independent starts/durations, or one fixed duration |
| `heavy_tailed`, `bursty`, `quantized` | Integer duration mixtures, clustered starts, or grid-aligned endpoint ties |
| `nested`, `staircase`, `multi_component` | Containment chains, regular sliding windows, or separated components with varying local geometry |
| `planted_balanced` | Balanced latent nonoverlapping classes with one common-point anchor per class |
| `star_components` | Separated stars, each with a long center and mutually disjoint contained leaves |
| `disjoint`, `clique`, `duplicates`, `with_empties` | Explicit controls, including configurable isolated empty intervals |

All endpoints are integers with half-open semantics `[start, end)`. Ordinary
families produce nonempty intervals. Every empty interval is an isolated vertex;
touching intervals do not overlap. `sorted` row order uses
`(start, end, original_index)`; `shuffled` uses a seeded permutation of the same
interval multiset. Presets include both orders.

`random_endpoints` samples two distinct coordinates from `[0, horizon]` per
interval; ties across intervals are allowed. For start/duration families,
`horizon` is the exclusive start window `[0, horizon)`: ends may extend beyond it
so durations remain intact. `quantized` uses grid-aligned starts and positive
grid-multiple durations within the requested inclusive duration range, rejecting
ranges with no such duration. `heavy_tailed` exposes duration scales and integer
weights, defaulting to `[1, 4, 16, 64]` and `[75, 18, 6, 1]`.

The planted construction validates its latent classes and anchor intersection
before discarding all class labels. Nonoverlapping classes prove an upper bound
of `k` on the chromatic number; the common anchor clique proves the matching
lower bound. Only the certificate is serialized: chromatic number `k` and minimum
possible class-size spread `0` when `n % k == 0`, otherwise `1`. Stars record their
leaf-count sequence and are checked geometrically without solving a partition
problem.

### Exhaustive JAIST import

Import local source files without network access, or explicitly download selected
orders into an ignored cache. Choose `all` or `connected` explicitly; the two
catalogs remain separate in provenance.

```sh
python benchmarks/generate_interval_graphs.py jaist --input /path/to/catalog --catalog-kind all --output benchmarks/generated/jaist-local
python benchmarks/generate_interval_graphs.py jaist --download --n 8 9 10 11 12 --catalog-kind all --cache benchmarks/generated/jaist-cache --output benchmarks/generated/jaist
```

Published per-order files are plain ASCII endpoint sequences, one graph per line,
with no header or count line. Despite its name,
`interval_disconnected_N.txt` contains all graphs; `interval_connected_N.txt`
contains only connected graphs. The combined `interval-1-9-*.txt` lists instead
have `size : N` section headers and `# of size N : COUNT` footers. Both formats
are accepted, including locally gzipped copies; declared section counts are
validated. Downloads discover the published links from the catalog page and
fail clearly for unavailable orders instead of guessing URLs.

The importer streams one graph at a time, with no graph-count cap or global
deduplication table. Provenance includes catalog kind, vertex count, sequential
zero-based catalog index, source filename, and SHA-256 digest. For a label
occurring at endpoint positions `p` and `q`, the parser emits `[p, q + 1)` and
sorts rows by catalog label. This
preserves the documented independent-set, path, and clique examples under
half-open semantics. Malformed encodings and inconsistent declared counts fail
with an error. Synthetic generation and tests never download data.

### Storage, statistics, and reproducibility

Each output directory contains `dataset.json` and
`shards/part-00000.jsonl.gz`, `part-00001.jsonl.gz`, and so on. Use `--shard-size`
to change the default 50,000 records per shard. A nonempty output directory is
rejected unless `--overwrite` is supplied. Bulk outputs and download caches under
`benchmarks/generated/` are ignored by git; do not commit generated corpora.

The manifest records `schema_version: 1`, corpus `kind`, `suite`, suite `seed`,
`instance_count`, `shard_size`, and ordered shard paths/counts. Each JSONL record
contains `id`, `family`, per-instance `seed` (null for JAIST), `n`, `params`,
`stats`, `certificate`, `provenance`, and `intervals` as `[start, end]` pairs.
`iter_dataset(path)` in the generator lazily yields these complete record dicts
for future benchmark consumers.

Statistics include vertex/nonempty/empty counts, overlap-edge count, edge
density, clique number `omega`, connected components, and endpoint bounds
(`null` for no endpoints). Independent `O(n log n)` sweeps count edges and
concurrency with END before START at equal coordinates. A sorted geometric scan
counts connected components, adding one per empty interval. No adjacency matrix
or production interval algorithm is used. For empty-only controls, `omega` records
the maximum nonempty concurrency, which is zero; the isolated vertices still
contribute to the vertex and component counts. Edge density uses all vertices.

SHA-256 over canonical identifying data derives stable IDs and per-instance
seeds. Geometry seeds depend on suite seed, family, parameters, and replicate;
row shuffles use separate deterministic seeds, so changing order preserves
geometry and adding another family does not change existing instances. Output
uses compact sorted-key JSON, stable record/shard order, and gzip with `mtime=0`
and no stored filename. Files contain no timestamps or host paths. Identical
generation commands in the same Python/zlib runtime produce byte-identical
datasets, including compressed shards.

Run the offline checks without building the extension:

```sh
uv run --locked --only-group dev ruff check benchmarks
uv run --locked --only-group dev ruff format --check benchmarks
uv run --locked --only-group dev python -m unittest discover -s benchmarks -p "test_generate_interval_graphs.py"
```

## Regenerate tables and optional plots

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

This reads saved measurements. It does not run benchmarks or compile Rust.
Commit regenerated files in `docs/assets/benchmarks/` with the source/configuration
change. The documentation CI build regenerates the assets before building the site.

Start a report from [report-template.md](report-template.md): Summary, Results,
Coverage and limitations, Reproduce and data. Register a small representative
table in [plots.toml](plots.toml), including important losses; a plot is optional.
See the [adding an operation checklist](../docs/benchmarking.md#adding-an-operation)
for scope lines, shared methodology, supporting notes, and navigation.
