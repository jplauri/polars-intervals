# polars-intervals

Fast interval algorithms for Polars, implemented in Rust.

This project is early-stage and not yet usable. No interval algorithms are
implemented yet.

## Architecture

- `crates/intervals-core` will contain Polars-independent interval algorithms.
  The crate has no dependencies and is imported in Rust as `intervals_core`.
- A later crate will provide Polars integration using `intervals-core`.
