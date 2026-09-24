# polars-intervals

Fast interval algorithms for Polars, implemented in Rust.

This project is early-stage. The core `overlap_counts` algorithm counts other
overlapping half-open intervals for each input row. Rust Polars users can call
`polars_intervals::overlap_count(&starts, &ends)` with matching non-null integer
Series (signed or unsigned, 8–64 bits) to obtain `UInt64` counts in input order.

Python 3.12+ users can pass column names or Polars expressions to
`polars_intervals.overlap_count`:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
result = df.lazy().with_columns(
    pi.overlap_count("start", "end").alias("count")
).collect()
# count: [1, 1, 2, 0] (UInt64)
```

Intervals are half-open `[start, end)`: touching endpoints do not overlap,
empty intervals count zero, and each row excludes itself. Duplicate non-empty
intervals count each other. Endpoints must have matching integer dtypes and no
nulls; `start > end` is invalid. Counts preserve row order and use the whole
input collection (or each group with `.over(...)`).

## Architecture

- `crates/intervals-core` contains Polars-independent interval algorithms.
  The crate has no dependencies and is imported in Rust as `intervals_core`.
- `crates/polars-intervals` adapts Polars Series to `intervals-core`.
  Its expression plugin powers the thin `python/polars_intervals` wrapper.
