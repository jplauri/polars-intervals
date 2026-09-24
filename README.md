# polars-intervals

Fast interval algorithms for Polars, implemented in Rust.

This project is early-stage and not yet usable with Polars. `overlap_counts` is
the first implemented core algorithm, counting other overlapping half-open
intervals for each input row.

## Architecture

- `crates/intervals-core` contains Polars-independent interval algorithms.
  The crate has no dependencies and is imported in Rust as `intervals_core`.
- `crates/polars-intervals` is the Polars expression-plugin scaffold.
  It depends on `intervals-core` and does not expose plugin functions yet.
