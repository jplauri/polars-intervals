# polars-intervals

Fast interval algorithms for Polars, implemented in Rust. The first operation,
`overlap_count`, counts how many **other** intervals overlap each row without
building a table of overlapping pairs.

This is an early-stage v0.1 project. The API and supported Polars versions may
change; `overlap_count` is currently the only public Python operation.

## Installation

To install a released version in a [uv](https://docs.astral.sh/uv/getting-started/installation/)-managed
project:

```sh
uv add polars-intervals
```

Or install with pip in your Python environment:

```sh
pip install polars-intervals
```

The first PyPI release is being prepared. For now, use the
[source installation](#source-and-development-installation) below.

The distribution is named `polars-intervals`; the import is `polars_intervals`.
Supported Python and Polars versions are declared in
[pyproject.toml](https://github.com/jplauri/polars-intervals/blob/master/pyproject.toml).

## Source and development installation

Building from source requires uv and
[Rust installed through rustup](https://www.rust-lang.org/tools/install), plus
your platform's native linker/build tools. Rustup selects the compiler from
`rust-toolchain.toml`.

```sh
git clone https://github.com/jplauri/polars-intervals.git
cd polars-intervals
uv sync --locked
uv run --locked python -c "import polars_intervals as pi; print(pi.overlap_count)"
```

This creates `.venv` and builds the Rust plugin through maturin. Run your scripts
with `uv run --locked python your_script.py`.

To use a local checkout from another uv-managed project, run this in that project:

```sh
uv add /path/to/polars-intervals
```

## Quick start

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
result = df.lazy().with_columns(pi.overlap_count("start", "end").alias("overlaps")).collect()
print(result["overlaps"].to_list())
# [1, 1, 2, 0]
```

`pi.overlap_count(start, end)` returns a **Polars expression**, with one non-null
`UInt64` count per row in the original order. It works in `select` and
`with_columns` on eager or lazy frames. Use `.alias(...)` to name the output.
Either argument can also be a Polars expression, for example
`pi.overlap_count(pl.col("start"), pl.col("end"))`.

Counts use the whole input collection. With
`pi.overlap_count("start", "end").over("group")`, comparisons stay within each
group.

## Interval semantics

Intervals are half-open: **`[start, end)`**. Two non-empty intervals overlap iff
`a.start < b.end and b.start < a.end`.

- `[1, 3)` and `[3, 5)` touch but do not overlap.
- `[1, 4)` and `[3, 5)` overlap.
- `[x, x)` is valid and empty: it overlaps nothing and receives zero.
- A row never counts itself. Identical non-empty intervals are separate rows
  and count each other.

## Inputs and errors

| Input | Requirement |
| --- | --- |
| `start`, `end` arguments | Column names (`str`) or `polars.Expr` |
| Endpoint columns | Same dtype: `Int8`, `Int16`, `Int32`, `Int64`, `UInt8`, `UInt16`, `UInt32`, or `UInt64` |
| Lengths | Equal; scalar expressions are not broadcast |
| Endpoint values | No nulls; `start <= end` in every row |

There is no automatic dtype conversion. Floats, strings, booleans, date/time
types, and other dtypes are unsupported. Any null endpoint is rejected, including
rows where both endpoints are null; null rows are not skipped or filled.

Invalid inputs raise a Polars error when the expression is evaluated (for a
lazy query, at `collect()`). A reversed interval (`start > end`) reports its
zero-based index in the input collection. Empty input with supported endpoint
dtypes returns an empty `UInt64` result.

## How it works

For `n` intervals, the Rust core sorts non-empty starts and ends independently
and uses binary searches to count overlaps in **O(n log n) time and O(n)
additional space**, preserving input order.

An inequality self-join followed by aggregation enumerates matching pairs.
Dense overlaps can create O(n²) pairs even though the final result has only `n`
counts. Avoiding that intermediate table can make `overlap_count` faster and use
less memory. Actual performance depends on the data, hardware, and query; see
the [reproducible benchmark methodology](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/README.md)
for the comparison and its limits.

## Architecture

- **`intervals-core`**: Rust interval algorithms with no production dependencies,
  generic over ordered endpoint types and independent of Polars and Python.
- **Polars plugin** (`crates/polars-intervals`): validates Polars inputs and
  adapts them to the core. Rust users can call
  `polars_intervals::overlap_count(&starts, &ends)` with Polars Series.
- **Python API** (`python/polars_intervals`): a thin typed wrapper that registers
  the Rust function as a Polars expression.

## Development and documentation

After `uv sync --locked`, run the Python checks:

```sh
uv run --locked ruff check .
uv run --locked ruff format --check .
uv run --locked pytest --doctest-modules python/polars_intervals tests
```

Run `uv run --locked ruff format .` to apply formatting.

Rust checks are `cargo fmt --check`, `cargo test --workspace --locked`, and
`cargo clippy --workspace --all-targets --locked -- -D warnings`.

Rust development and CI use the stable compiler pinned in
`rust-toolchain.toml`. Older compilers are untested; no separate minimum supported
Rust version (MSRV) is promised. Compiler updates should pass the Rust and compiled
Python plugin checks before merging.

Check the public Rust documentation with
`cargo doc --workspace --no-deps --locked` and `RUSTDOCFLAGS="-D warnings"` in the
environment. The generated Rust API reference is written to `target/doc/`.

The checkout is installed in editable mode, so Python edits are available
immediately. Run `uv sync --locked` after changing Rust code to rebuild the plugin.
Commit the lockfiles when updating dependencies.

The documentation includes this README and an API reference generated from the
`overlap_count` docstring. Edit the function's docstring to update its API
documentation. Preview or build locally with:

```sh
uv run --locked --isolated --only-group docs mkdocs serve
uv run --locked --isolated --only-group docs mkdocs build --strict
```

Building the docs does not require compiling the Rust plugin. The generated site
is written to `target/docs/`. Check the API examples against the installed plugin
with `uv run --locked pytest --doctest-modules python/polars_intervals`.
