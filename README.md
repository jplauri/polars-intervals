# polars-intervals

Fast interval algorithms for Polars, implemented in Rust.

This project is early-stage. The core `overlap_counts` algorithm counts other
overlapping half-open intervals for each input row. Rust Polars users can call
`polars_intervals::overlap_count(&starts, &ends)` with matching non-null integer
Series (signed or unsigned, 8–64 bits) to obtain `UInt64` counts in input order.
The Python package is a scaffold; `overlap_count` is not exposed in Python yet.

## Architecture

- `crates/intervals-core` contains Polars-independent interval algorithms.
  The crate has no dependencies and is imported in Rust as `intervals_core`.
- `crates/polars-intervals` adapts Polars Series to `intervals-core`.
  It does not expose expression plugin entrypoints yet.
