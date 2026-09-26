# Release notes

## Unreleased

- Assign intervals to the fewest non-overlapping lanes with
  [`assign_lanes`](usage.md#assign-the-minimum-number-of-lanes).
- Select a schedule with maximum total weight using
  [`max_weight_non_overlapping`](usage.md#select-a-globally-maximum-weight-schedule),
  or allow limited overlap with
  [`max_weight_with_capacity`](usage.md#select-with-a-simultaneous-capacity).
- Cover a target with the fewest intervals using
  [`minimum_cover`](usage.md#cover-one-continuous-target), or minimize its cost
  with [`minimum_cost_cover`](usage.md#cover-at-minimum-cost).
- Find the fewest points that hit every interval with
  [`minimum_stabbing_points`](usage.md#minimum-stabbing-points).
- Use matching `Date` or `Datetime` endpoints as well as integers.
- Browse [benchmark reports and plots](benchmarks.md) for each operation.

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
