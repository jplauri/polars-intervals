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

```sh
uv run --locked ruff check .
uv run --locked ruff format --check .
uv run --locked pytest --doctest-modules python/polars_intervals tests
uv run --locked --only-group dev pytest .github/scripts/test_*.py
uv lock --check
cargo fmt --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use `uv run --locked ruff format .` to format Python code.
CI also verifies built distributions when code or build inputs change.

## Documentation

Edit pages in `docs/` and API docstrings in `python/polars_intervals/`.

```sh
uv run --locked --isolated --only-group docs mkdocs serve
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The site is written to `target/docs/`. Building it does not compile the Rust
plugin. See [Running and publishing benchmarks](https://github.com/jplauri/polars-intervals/blob/master/docs/benchmarking.md)
for report templates, plotting commands, and reporting checks.

Check API examples against the installed plugin with:

```sh
uv run --locked pytest --doctest-modules python/polars_intervals
```

For the Rust API reference, set `RUSTDOCFLAGS` to `-D warnings` and run
`cargo doc --workspace --no-deps --locked`. The output is written to `target/doc/`.

## Repository layout

| Location | Purpose |
| --- | --- |
| `crates/intervals-core` | Interval algorithms, independent of Polars and Python |
| `crates/polars-intervals` | Polars input validation, Rust API, and expression plugin |
| `python/polars_intervals` | Python expressions and API docstrings |
| `tests` | Python integration tests |
| `benchmarks` | Benchmark runners, reporting tools, and saved results |
| `docs` | Documentation site and release guide |

The generic `intervals_core::containment_counts(&starts, &ends)` API has no
production dependencies. `polars_intervals::containment_count(&starts, &ends)`
uses the shared integer, Date, and Datetime extraction path.

For publishing instructions, see the
[release guide](https://github.com/jplauri/polars-intervals/blob/master/docs/releasing.md).
