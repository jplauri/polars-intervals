# Maximum-weight clique verification — 2026-09-28

Verified on Windows 11 x64, Rust 1.98.1, Python 3.14.0 and Polars 1.44.2,
using the repository-pinned toolchain and lockfiles. The base revision was
`023174dfc50bef6993dd225f125064ba64fab07d`; feature changes and unrelated existing
working-tree edits were present. Run metadata records measured source hashes.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 389 tests and doctests passed with the Windows linker workaround below |
| `PROPTEST_CASES=2048 cargo test -p intervals-core --test max_weight_clique --locked` | All 26 tests passed, including 9 property tests |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | Passed |
| `uv run --locked --no-sync ruff check .` | Passed |
| `uv run --locked --no-sync ruff format --check .` | 98 files passed |
| `uv run --locked --no-sync pytest --doctest-modules python/polars_intervals tests` | 2,558 passed against the final release extension |
| `.github/scripts/test_*.py` checks from CONTRIBUTING | 58 passed |
| `uv lock --check` | Passed |
| Benchmark asset generation | Passed; all 44 preexisting assets remained byte-identical |
| Reporting unit tests | 6 passed |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed |
| Rendered report review | Passed at 1440×1000 and 390×844; mobile tables scroll horizontally without widening the page |

The first workspace test invocation hit MSVC `LNK1140` while linking the existing
Polars doctests because the default program database exceeded the linker's size
limit. The full workspace rerun passed with
`RUSTDOCFLAGS="-C link-arg=/PDB:NONE"`; that option redirects the PDB to a file
named `NONE`, which was removed afterwards. A cleaner follow-up disabled debug
link output with `RUSTDOCFLAGS="-C link-arg=/DEBUG:NONE"` and
`cargo test --workspace --locked --doc`: all 20 doctests passed. No source change
or test exclusion was needed. The Python DLL search-path setup and
`CARGO_BUILD_JOBS=2` followed CONTRIBUTING.

The final extension was rebuilt after all production Rust edits:

```powershell
$env:CARGO_BUILD_JOBS = "2"
uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"
```

The rebuild took 13m 44s. Subsequent Python commands used `--no-sync` to retain it.
The benchmark runner checks the installed extension's SHA256 against the Cargo
release library and rejects newer Rust inputs before collecting measurements.

## Benchmark evidence

| Run | Raw samples | Workloads | Scope |
| --- | ---: | ---: | --- |
| `clique-core-exploratory-windows-20260928` | 9,384 | 368 | Initial exploration; initial production source hash retained separately |
| `clique-core-windows-20260928` | 65,880 | 1,532 | Final production, 5 samples and 2 warmups per candidate |
| `clique-core-repeat-windows-20260928` | 700 | 20 | Separate repeat, 7 samples and 3 warmups |
| `clique-polars-windows-20260928` | 3,080 | 616 | Final release plugin, 5 samples and 2 warmups, seeds 42/137, one Polars thread |

All timed core candidates passed untimed independent objective and feasibility
checks. Small exhaustive pairwise-subset tests and an independent quadratic
reference cover every private candidate. Large masks use linear common-point
checks. Allocator measurements are separate from timing and report requested
live heap, not RSS. Core timings include result destruction; Polars timings
exclude it. Runs are not pooled.

Every Polars workload passed an independent per-group objective calculation,
positive-only selection, linear feasibility, length/dtype/null and repeated-mask
checks. Implicit units matched explicit ones. Installed and release binary
SHA256 values both equal
`9b65cabcf79cc4199c1d0792be889554e6792b5f13e752c164a9dce8e2189765`.

The [report](../../docs/clique-benchmarks.md) gives representative wins, losses,
memory tradeoffs and exact coverage limits. This is one Windows machine with
synthetic fixtures. Only selected core families reach one million rows;
quadratic candidates stop at 64, and coordinate compression was not implemented.

## Feature files

- Generic implementation and exports: `crates/intervals-core/src/clique.rs`,
  `crates/intervals-core/src/lib.rs`.
- Adapter/plugin and Python expression: `crates/polars-intervals/src/lib.rs`,
  `python/polars_intervals/__init__.py`.
- Tests: both crates' `tests/max_weight_clique.rs`, core
  `tests/clique_candidates.rs`, and `tests/test_max_weight_clique.py`.
- Private candidates and runners: core `benches/max_weight_clique.rs`,
  `benches/support/clique.rs`, its Cargo benchmark registration, and
  `benchmarks/max_weight_clique.py`.
- Documentation: README, usage/API pages, benchmark overview/navigation,
  `docs/clique-benchmarks.md`, `benchmarks/clique-notes.md`, benchmark inventory,
  two reporting tables in `benchmarks/plots.toml`, four generated table assets,
  and the saved clique results/metadata/verification files.

Unrelated preexisting lane-balancing and benchmark-reporting work was preserved.
