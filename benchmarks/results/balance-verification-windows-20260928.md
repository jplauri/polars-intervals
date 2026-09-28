# Lane balancing verification, 2026-09-28

Local Windows 11 x86-64 checkout at revision
`023174dfc50bef6993dd225f125064ba64fab07d` with uncommitted feature and pre-existing
documentation changes. Python 3.14.0, Polars 1.44.2, Rust/Cargo 1.98.1.
Run metadata beside the measurements records source/native hashes and dirty status.

The following completed successfully on the implementation used for measurement:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo test --workspace --locked` | 353 tests/doctests passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --locked` | Passed (PowerShell environment syntax used) |
| `uv run --locked ruff check .` | Passed |
| `uv run --locked ruff format --check .` | 90 files already formatted |
| `uv run --locked pytest --doctest-modules python/polars_intervals tests` | 2,404 passed |
| `uv lock --check` | Passed |
| `uv run --no-sync pytest .github/scripts -q` | 58 passed |
| `python -m unittest discover -s benchmarks -p test_generate_interval_graphs.py` | 36 offline tests passed |
| `python -m unittest discover -s benchmarks -p test_balance_lanes.py` | 18 offline runner tests passed |
| `uv run --locked --isolated --only-group plots python -m unittest discover -s benchmarks -p test_plot.py` | 6 passed |
| `uv sync --locked --reinstall-package polars-intervals --config-setting "build-args=--profile release"` | Release plugin installed successfully |
| `uv run --no-sync pytest --doctest-modules python/polars_intervals tests` | Release-plugin recheck: 2,404 passed |
| `cargo bench -p intervals-core --bench balance_lanes --locked` | 60 fixtures; 780 quality/options records and 3,900 validated samples |
| `uv run --no-sync python benchmarks/balance_lanes.py ...` | All three recorded runs completed: 541/132/40 instances; exact commands in supporting notes and metadata |
| `uv run --locked --isolated --only-group plots python benchmarks/plot.py` | All tables regenerated, including both balancing tables |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed |

The two offline unittest commands used the repository virtual environment's
Python directly, without installing or rebuilding the plugin. The feature's
Python integration file also passed separately: 427 cases. Core tests include
capped exhaustive/orientation oracles and persisted proptest conventions.

Windows Rust adapter tests require `PYO3_PYTHON` to point at the virtual
environment's Python and its `sys.base_prefix` directory on `PATH` for the Python
runtime DLL. Builds used `CARGO_BUILD_JOBS=2`. Earlier concurrent Cargo/doc-test
attempts hit stale dependency artifacts (`E0460`/missing rlib); the final serial
clippy, rustdoc and full workspace test runs above passed. A supplemental
`unittest` discovery in `.github/scripts` found no unittest cases; the correct
pytest command above ran all 58 tests.
The first core benchmark invocation used relative output paths, which Cargo
resolved from the crate directory; it failed before writing measurements. The
recorded successful invocation used absolute output paths, now documented.

Cross-platform CI and full remote JAIST catalogs were not run locally. Local
JAIST tests and measurements use checked-in tiny fixtures and make no network
requests. No global-balance guarantee is inferred from tests or measurements.

Rendered report inspection used preinstalled headless Microsoft Edge against the
local strict-build output at desktop (1,365 px) and narrow (600 px) widths. Both
showed readable prose and tables. The in-app browser tool could not initialize
(`failed to write kernel assets`, Windows error 3), so headless rendering provided
the visual check. Screenshots and temporary browser profiles are under ignored
`target/balance-work/`, not committed artifacts. A final no-sync Ruff pass and
format check passed (94 files), and the lockfile remained current.
