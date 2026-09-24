# polars-intervals

Fast interval algorithms for Polars, implemented in Rust. The first operation,
`overlap_count`, counts how many **other** intervals overlap each row without
building a table of overlapping pairs.

This is an early-stage v0.1 project. The API and supported Polars versions may
change; `overlap_count` is currently the only public Python operation.

## Installation

For the current package, build from this repository. You need **Python 3.12+**,
[uv](https://docs.astral.sh/uv/getting-started/installation/), and a current stable
[Rust toolchain](https://www.rust-lang.org/tools/install) with your platform's
native linker/build tools. The Python package requires **Polars >=1.44.1,<1.45**;
uv installs the compatible version from `uv.lock`.

```sh
git clone https://github.com/jplauri/polars-intervals.git
cd polars-intervals
uv sync --locked
uv run --locked python -c "import polars_intervals as pi; print(pi.overlap_count)"
```

This creates `.venv` and builds the Rust plugin through maturin. Run your scripts
with `uv run --locked python your_script.py`. The distribution is named
`polars-intervals`; the import is `polars_intervals`.

To use a local checkout from another uv-managed project, run this in that project
(which must also use Python 3.12+):

```sh
uv add /path/to/polars-intervals
```

## Quick start

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
result = df.lazy().with_columns(
    pi.overlap_count("start", "end").alias("overlaps")
).collect()
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

After `uv sync --locked`, run `uv run --locked pytest` for Python integration
tests. Rust checks are `cargo fmt --check`, `cargo test --workspace`, and
`cargo clippy --workspace --all-targets -- -D warnings`.

The checkout is installed in editable mode: Python edits are available
immediately, and edits to either Rust crate trigger a rebuild on the next
`uv sync` or `uv run`.
Commit `uv.lock` when updating dependencies.

The documentation uses Material for MkDocs. Its home page includes this README;
the API reference is generated from the typed `overlap_count` signature and its
Google-style docstring. Edit the function's docstring to update parameters,
returns, errors, and runnable examples. Preview or build locally with:

```sh
uv run --locked --isolated --only-group docs mkdocs serve
uv run --locked --isolated --only-group docs mkdocs build --strict
```

These commands install only the docs tools in an isolated environment and read
the Python source without importing the compiled plugin. The generated site is
written to `target/docs/`. Check the API examples against the installed plugin
with `uv run --locked pytest --doctest-modules python/polars_intervals`.
