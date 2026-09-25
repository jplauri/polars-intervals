# Release notes

## 0.1.0 — September 25, 2026

First release of polars-intervals. Count overlapping intervals directly in
your Polars queries.

```sh
pip install polars-intervals==0.1.0
```

- Use `overlap_count` in eager or lazy queries, including within groups.
- Get one count per interval, excluding the interval itself.
- Compute counts without building a table of overlapping pairs.

Intervals are half-open: touching endpoints do not overlap, and empty
intervals count zero. Endpoints must be non-null integers with matching dtypes.

Requires Python 3.12+ and Polars `>=1.44.1,<1.45`.
The API is early-stage and may change.

See [Getting started](index.md), [Usage](usage.md), or the
[API reference](api.md).
