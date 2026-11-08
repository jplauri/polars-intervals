# Contributing

## Build from source

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) and
[Rust through rustup](https://www.rust-lang.org/tools/install), plus your
platform's native linker and build tools. Rustup selects the compiler from
`rust-toolchain.toml`.

```sh
git clone https://github.com/jplauri/polars-intervals.git
cd polars-intervals
uv sync --locked
uv run --locked python -c "import polars_intervals as pi; print(pi.overlap_count)"
```

This creates `.venv` and builds the Rust plugin through maturin. Run scripts
with `uv run --locked python your_script.py`.

Python edits take effect immediately. Run `uv sync --locked` after changing Rust
code to rebuild the plugin. Commit the lockfiles when updating dependencies.

To use a local checkout from another uv project, run `uv add /path/to/polars-intervals`
in that project.

## Checks

Run the checks relevant to your change. The standard checks are:

```sh
uv run --locked ruff check .
uv run --locked ruff format --check .
uv run --locked pytest --doctest-modules python/polars_intervals tests
uv run --locked python -m pytest benchmarks/test_provenance.py benchmarks/test_coverage_profile.py benchmarks/test_coverage_stats.py benchmarks/test_interval_geometry.py benchmarks/test_set_geometry.py
uv run --locked --only-group dev pytest .github/scripts/test_*.py
uv lock --check
cargo fmt --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use `uv run --locked ruff format .` to format Python code.
CI also verifies built distributions when code or build inputs change.

On Windows, point PyO3 at the project interpreter before running Cargo. In
PowerShell, prepare the shell after `uv sync --locked`:

```powershell
$env:PYO3_PYTHON = (Resolve-Path .venv/Scripts/python.exe).Path
$env:CARGO_BUILD_JOBS = "2"
```

The last setting also matches release CI and bounds compilation memory usage.

## Documentation

Edit pages in `docs/` and API docstrings in `python/polars_intervals/`.
Use an isolated environment to preview the site or check the build. The first
command generates the benchmark tables that the pages include:

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group docs mkdocs serve
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The site is written to `target/docs/`. These commands do not compile the Rust
plugin. The **Documentation** workflow validates every pull request and push to
`master`, and publishes successful `master` builds to
[GitHub Pages](https://jplauri.github.io/polars-intervals/). Pull requests provide
a downloadable `github-pages` artifact for previewing the site. A manual run
on `master` can republish it; runs on other branches only validate and build.

The site tracks `master` without versioned snapshots, so documentation fixes
publish without a package release. GitHub Pages must use **GitHub Actions** as
its source in **Settings > Pages**. The `github-pages` environment should allow
deployments only from the `master` branch. No generated HTML is committed.

See [Running and publishing benchmarks](https://jplauri.github.io/polars-intervals/benchmarking/)
for report templates, plotting commands, and reporting checks.

Check API examples against the installed plugin with:

```sh
uv run --locked pytest --doctest-modules python/polars_intervals
```

For the `intervals-core` API reference, set `RUSTDOCFLAGS` to `-D warnings` and run
`cargo doc --workspace --no-deps --locked`. The output is written to `target/doc/`.

## Repository layout

| Location | Purpose |
| --- | --- |
| `crates/intervals-core` | Interval algorithms, independent of Polars and Python |
| `crates/polars-intervals` | Polars input validation and expression plugin |
| `python/polars_intervals` | Python expressions and API docstrings |
| `tests` | Python integration tests, which also test the plugin adapter |
| `benchmarks` | Benchmark runners, reporting tools, and saved results |
| `docs` | Documentation site and release guide |

The generic `intervals_core::containment_counts(&starts, &ends)` API has no
production dependencies. The adapter crate has no public Rust API. Python is
its only caller, so the adapter is tested from Python only.

For publishing instructions, see the
[release guide](https://github.com/jplauri/polars-intervals/blob/master/docs/releasing.md).
