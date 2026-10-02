# Release notes

## Unreleased

## 0.3.0 — October 2, 2026

Measure coverage for each query window, build interval unions and gaps, and
compare two interval collections. Add coverage profiles, exact interval
selection, and balanced lane assignment.

```sh
pip install polars-intervals==0.3.0
```

Requires Python 3.12+ and Polars `>=1.44.1,<1.45`.
Existing Python signatures and result semantics are preserved. Endpoint dtypes
must match exactly and endpoints must be non-null. See [Usage](usage.md) for
each operation's empty-interval rules. The API remains early-stage and may change.

- Add [`coverage_stats`](coverage-stats.md) for source-record counts and exact
  union coverage inside every query window. Preserve all query rows and payloads
  with eager, lazy, and mixed inputs. Lengths use Int128, and fractions are null
  only for empty queries. Shared nullable keys support independent groups.

- Add [`subtract_intervals` and `intersect_intervals`](interval-geometry.md#subtract-and-intersect-two-collections)
  for exact set difference and geometric intersection of two collections.
  Both support grouped eager, lazy and mixed inputs, exact logical dtypes,
  and validation of every row on both sides.

- Add [`cluster_intervals`, `merge_intervals`, and `interval_gaps`](interval-geometry.md)
  for connected records, exact union, and complement inside explicit scalar
  bounds. All support eager and genuinely lazy execution with exact endpoint
  dtypes. Empty rows are isolated clusters and contribute no coverage.

- Add [`coverage_profile`](coverage-profile.md) for coverage-depth and resource-load
  segments with exact Int128 loads. Support grouped eager and lazy inputs,
  explicit observation bounds, and optional zero-load gaps.

- Add [`assign_balanced_lanes`](usage.md#balance-lane-row-counts) to improve lane
  row-count balance within a configurable work budget. The minimum lane count
  is guaranteed. Balance improvements are heuristic.

- Add [`max_weight_clique`](usage.md#select-a-maximum-weight-clique) to select an
  exact maximum-weight clique, using unit weights by default.

- Add [`minimum_cost_dominating_set`](usage.md#select-a-minimum-cost-dominating-set)
  to select representative intervals at minimum total cost, then minimum row
  count. Omitted costs minimize row count.

- Publish the documentation site and clarify task guides, execution behavior,
  validation, and benchmark reports.

- **Breaking Rust API change:** In the unpublished `intervals-core` crate,
  consolidate length-mismatch error variants into
  `IntervalError::LengthMismatch([(&'static str, usize); 2])`.
  New error variants also require updates to exhaustive matches.

- Operations using shared endpoint validation now report the first row containing
  a null endpoint.

## 0.2.0 — September 27, 2026

Expand the native interval APIs with containment, nesting, scheduling, covering,
and coverage operations, plus Date and Datetime endpoint support.

```sh
pip install polars-intervals==0.2.0
```

Requires Python 3.12+ and Polars `>=1.44.1,<1.45`.
Endpoint dtypes must match exactly and endpoints must be non-null. Empty
interval semantics depend on the operation; see [Usage](usage.md).
The API remains early-stage and may change.

- Add [`nesting_depth`](usage.md#nesting-depth): exact, row-aligned longest strict
  containment chain depth, with outermost depth zero and duplicate geometries
  sharing the same level. Supports grouped integer, Date and Datetime endpoints.

- Select a globally maximum-weight subset under a piecewise-constant capacity
  profile with [`max_weight_with_capacity_profile`](usage.md#select-with-a-capacity-profile),
  an eager API backed by exact native flow optimization, accepting a separate
  profile DataFrame or profile columns in the jobs DataFrame.

- Add `max_k_coverage`: exact maximum union measure with a scalar interval
  budget, using the fewest intervals among maximum-coverage solutions.

- Count other contained intervals per row with
  [`containment_count`](usage.md#count-containment), including duplicates,
  empty intervals, grouped windows, and matching integer/Date/Datetime endpoints.

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
