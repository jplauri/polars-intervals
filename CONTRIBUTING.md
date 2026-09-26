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

Pull requests that change only `README.md`, `CONTRIBUTING.md`, `mkdocs.yml`,
`docs/`, `benchmarks/README.md`, or `benchmarks/results/` run lint, formatting,
documentation, and CI helper checks without compiling Rust or building wheels.
Python docstrings, package metadata, dependencies, workflow changes, and all other
paths trigger full checks. Required Python and Rust checks still report a result.

Branch pushes are checked through their pull request. Pushes to `master` always
run full Python and Rust checks, as required by the release guard. Manual release
rehearsals and published releases always build and verify every distribution.

Updating a pull request cancels its older CI runs. Python and Rust checks cache
compiled dependencies; wheel builds use sccache. The first run populates these
caches. A manual release rehearsal on `master` seeds wheel caches for future PRs.

Rust development and CI use the stable compiler pinned in `rust-toolchain.toml`.
Older compilers are untested; no separate minimum supported Rust version is
promised. Compiler updates should pass the Rust and compiled Python plugin checks.

## Documentation

The Python API reference comes from the public function docstrings. Edit those
to update the reference; put usage examples in `docs/usage.md`.

```sh
uv run --locked --isolated --only-group docs mkdocs serve
uv run --locked --isolated --only-group docs mkdocs build --strict
```

The site is written to `target/docs/`. Building it does not compile the Rust
plugin. Check the API examples against the installed plugin with:

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
| `benchmarks` | Algorithm comparisons and runtime/memory comparisons with native Polars |
| `docs` | Documentation site and release guide |

The core supports ordered endpoint types and has no production dependencies.
The Rust Polars API exposes `polars_intervals::overlap_count(&starts, &ends)`
and `polars_intervals::assign_lanes(&starts, &ends)`, plus
`polars_intervals::max_weight_non_overlapping(&starts, &ends, &weights)`
for matching integer, Date, or Datetime Series.

The core lane comparison needs no Polars build:

```sh
cargo bench -p intervals-core --bench assign_lanes --locked
cargo bench -p intervals-core --bench max_weight_non_overlapping --locked
cargo bench -p intervals-core --bench covering --locked
```

It validates all candidates before timing. See the
[methodology and results](https://github.com/jplauri/polars-intervals/blob/master/docs/assign-lanes-benchmarks.md).

Exact covering is exposed as `minimum_cover` and `minimum_cost_cover` in each
layer. Rust Polars takes `&Scalar` target endpoints with exactly matching dtypes;
the independent core takes two scalar values of its generic endpoint type.
The [covering comparison](https://github.com/jplauri/polars-intervals/blob/master/docs/covering-benchmarks.md)
records candidate selection, phase timings, and memory measurements.

For publishing instructions, see the
[release guide](https://github.com/jplauri/polars-intervals/blob/master/docs/releasing.md).
