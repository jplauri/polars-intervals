# Release notes

## Unreleased

- Add exact `minimum_cover` and `minimum_cost_cover` to Python, Rust Polars, and
  the independent core. Cover one continuous half-open scalar target with the
  fewest intervals or minimum nonnegative integer cost (then fewest intervals).
  Empty targets select nothing; infeasible targets raise. Scalar temporal metadata
  is validated exactly, and groups independently cover the same target.
- Compare packed/indirect greedy sweeps, a heap reference, Fenwick/segment-tree
  frontier DP, and a quadratic reference in release mode. Add exhaustive subset
  oracles, proptest invariants and metamorphic properties, native integration tests,
  and reproducible phase/allocation measurements through one million intervals.

- Add exact `max_weight_with_capacity` selection to Python, Rust Polars and the
  independent core, with integer weights, half-open intervals, free positive
  empty intervals, and independent group/window optimization. Capacity one uses
  the existing weighted scheduler; general capacities use interval min-cost flow.
- Add exact flow candidates, component comparisons, release benchmarks, exhaustive
  and property-based optimality checks, and residual-network invariant tests.

- Add exact `max_weight_non_overlapping` scheduling to Python, Rust Polars, and
  the independent core, with a Boolean mask, integer weights and checked `i128`
  accumulation. Supports integer/Date/Datetime endpoints, groups and empties.
- Compare three weighted scheduling candidates with release benchmarks, an
  independent suffix DP, brute-force enumeration and property tests.

- Add `assign_lanes` to the Python expression, Rust Polars, and independent core
  APIs, returning optimal, deterministic, contiguous `UInt32` lane IDs in row order.
  Supports the same integer, Date, and Datetime endpoints as `overlap_count`.
- Compare three exact algorithms in release mode; retain reproducible candidates,
  independent optimality checks, property tests, and measured selection rationale.

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
