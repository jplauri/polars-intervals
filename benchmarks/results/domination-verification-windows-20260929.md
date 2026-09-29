# Dominating-set verification record

Implemented in the working tree based on `d742de3e57fdc523d1673e81106ca6d5109127bb`.
Run-specific source hashes, environment details, commands and CSV hashes are
in the adjacent `domination-*.metadata.json` files. No dependencies or versions
were changed. Existing covering algorithms remain unchanged.

## Checks run

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 421 Rust tests and doctests passed |
| `PROPTEST_CASES=2048 cargo test -p intervals-core --locked --test minimum_cost_dominating_set` | All 24 tests passed against final production; 8 proptest properties honor the higher case count |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed, including new benchmarks |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --locked` | Passed |
| `uv run --locked --no-sync ruff check .` | Passed |
| `uv run --locked --no-sync ruff format --check .` | Passed |
| `uv run --locked --no-sync pytest --doctest-modules python/polars_intervals tests` | 2,688 passed, including 129 new Python integration cases and all existing covering/clique tests |
| CI helper tests (`test_release_checks.py`, `test_ci_changes.py`) | 58 passed; file arguments expanded explicitly on Windows |
| `uv lock --check` | Passed |
| `uv run --locked --isolated --only-group plots python benchmarks/plot.py` | Generated tables from saved raw data |
| `uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py` | All 6 reporting tests passed |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed |
| Rendered benchmark page | Inspected at 1440px and 390px; text and tables render, narrow tables scroll within their wrappers without widening the page |

The native extension was rebuilt using
`uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"`.
Checks after this used `--no-sync` to preserve that release build. The Polars
runner verified that the installed extension and local release library have
identical hashes and that no native source input is newer than the library.
On Windows, Cargo checks used the documented `PYO3_PYTHON`, Python DLL runtime
path and `CARGO_BUILD_JOBS=2` setup.

## Measurements and interpretation

- Pilot and large comparison measured A (existing covering), fused A, B (heap),
  C (unit/uniform greedy), and a bounded quadratic reference. Their
  `production` label denotes the initial covering baseline.
- Final production matrix: 15,100 samples, sizes 0 through 100k, Int64/UInt64,
  two seeds, five samples after two warmups, including duplicate geometries
  with costs 1 versus 1,000,000.
- Final million-row repeat: 210 samples across seven focused families, Int64,
  two seeds, final production versus covering and fused covering.
- Final release Polars matrix: 2,640 collection samples through 100k rows,
  one thread, two seeds, five samples after two warmups, integer/temporal,
  groups, mismatched chunks, and streaming execution.
- Every timed output passed untimed original-graph feasibility checks. CSV
  evidence labels distinguish subset/analytic optima, cross-candidate agreement,
  and feasibility-only checks. These are not interchangeable guarantees.
- Peak requested live heap and allocation counts were measured separately.
  Returned-result destruction is excluded from timings; internal cleanup is
  included. Inputs, allocator overhead, stack and process RSS are excluded
  from the allocator measurement.

The final repeat supports heap DP on general costs and direct greedy on
unit/uniform costs. It retains measured losses on cliques, nesting and reversed
unit duplicates. Million-row Polars, process RSS, other operating systems,
candidate coalescing and the optional literature challenger were not measured.
See the report and design notes for proofs, exact results and limitations.

## After the Ponytail pass and integration with master

The measured implementation is preserved in commit `510cd44`. The feature was
then integrated with `edf743b`, including shared validation, Python registration,
benchmark provenance and endpoint test tables. Duplicate RNG code and dead
forwarding helpers were removed. Historical raw results and metadata were not
rewritten, and all five recorded CSV hashes were verified unchanged.

The following checks were rerun on the integrated sources:

| Check | Result |
| --- | --- |
| Workspace Rust tests and doctests | 395 passed; upstream cleanup consolidated existing tests |
| Domination proptests with `PROPTEST_CASES=2048` | All 24 tests passed |
| Workspace formatting, all-target Clippy, warnings-as-errors Rust documentation | Passed |
| Release plugin rebuild | Passed using the explicit release command above |
| Full Python suite and docstrings against the rebuilt plugin | 2,950 passed, including shared endpoint validation |
| Ruff check/format, lockfile check and CI helper tests | Passed |
| Table generation, six reporting tests and strict documentation build | Passed |
| Core runner smoke, sizes 0 and 8, both integer dtypes, all eligible candidates | 478 samples checked successfully |
| Release Polars runner smoke, sizes 0 and 8, all cases/cost modes | 88 samples checked successfully, including installed/release library hash equality |

Smoke outputs live under ignored `target/` paths and are correctness checks,
not replacements for the published performance measurements.
