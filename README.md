# polars-intervals

Interval algorithms for Polars, with support for eager and lazy queries.

[Documentation](https://jplauri.github.io/polars-intervals/) ·
[Usage](https://jplauri.github.io/polars-intervals/usage/) ·
[Benchmarks](https://jplauri.github.io/polars-intervals/benchmarks/)

## Install

Requires Python 3.12+.

```sh
pip install polars-intervals
```

Or with uv: `uv add polars-intervals`.

## Algorithms

### Counting and nesting

| Function | Description |
| --- | --- |
| [`overlap_count`](https://jplauri.github.io/polars-intervals/usage/#count-overlaps) | Count the other intervals overlapping each interval. |
| [`coverage_profile`](https://jplauri.github.io/polars-intervals/coverage-profile/) | Return segments showing coverage depth or resource load. |
| [`containment_count`](https://jplauri.github.io/polars-intervals/usage/#count-containment) | Count the other intervals contained by each interval. |
| [`nesting_depth`](https://jplauri.github.io/polars-intervals/usage/#nesting-depth) | Length of the longest strict containment chain above each interval. |

### Interval geometry

| Function | Description |
| --- | --- |
| [`subtract_intervals`](https://jplauri.github.io/polars-intervals/interval-geometry/#subtract-and-intersect-two-collections) | Remove one collection's coverage from another. |
| [`intersect_intervals`](https://jplauri.github.io/polars-intervals/interval-geometry/#subtract-and-intersect-two-collections) | Return coordinates covered by both collections. |
| [`cluster_intervals`](https://jplauri.github.io/polars-intervals/interval-geometry/) | Label connected records in original row order, optionally joining touching intervals. |
| [`merge_intervals`](https://jplauri.github.io/polars-intervals/interval-geometry/) | Return the exact union as maximal ranges, always joining touching ranges. |
| [`interval_gaps`](https://jplauri.github.io/polars-intervals/interval-geometry/) | Return uncovered ranges inside required scalar bounds. |

### Lane assignment

| Function | Description |
| --- | --- |
| [`assign_lanes`](https://jplauri.github.io/polars-intervals/usage/#assign-the-minimum-number-of-lanes) | Assign the fewest lanes with no overlaps within a lane. |
| [`assign_balanced_lanes`](https://jplauri.github.io/polars-intervals/usage/#balance-lane-row-counts) | Assign the fewest lanes and heuristically balance row counts, optionally improving supplied `initial_lanes`. |

### Weighted selection

| Function | Description |
| --- | --- |
| [`max_weight_non_overlapping`](https://jplauri.github.io/polars-intervals/usage/#select-a-globally-maximum-weight-schedule) | Select a non-overlapping subset with maximum total weight. |
| [`max_weight_clique`](https://jplauri.github.io/polars-intervals/usage/#select-a-maximum-weight-clique) | Select one maximum-weight set of pairwise intersecting intervals. Omitted weights mean units. |
| [`max_weight_with_capacity`](https://jplauri.github.io/polars-intervals/usage/#select-with-a-simultaneous-capacity) | Maximize total weight under a simultaneous overlap limit. |
| [`max_weight_with_capacity_profile`](https://jplauri.github.io/polars-intervals/usage/#select-with-a-capacity-profile) | Maximize total weight under a piecewise-constant capacity profile. |

### Coverage and stabbing

| Function | Description |
| --- | --- |
| [`max_k_coverage`](https://jplauri.github.io/polars-intervals/usage/#select-maximum-coverage-with-a-budget) | Maximize the total measure covered by at most `k` intervals. |
| [`minimum_cover`](https://jplauri.github.io/polars-intervals/usage/#cover-one-continuous-target) | Cover a target with the fewest intervals. |
| [`minimum_cost_cover`](https://jplauri.github.io/polars-intervals/usage/#cover-at-minimum-cost) | Cover a target at minimum total cost. |
| [`minimum_cost_dominating_set`](https://jplauri.github.io/polars-intervals/usage/#select-a-minimum-cost-dominating-set) | Select minimum-cost representatives that dominate every interval vertex. |
| [`minimum_stabbing_points`](https://jplauri.github.io/polars-intervals/usage/#minimum-stabbing-points) | Find the fewest points that hit every interval. |

## Examples

### Count overlaps

Count how many other intervals overlap each row:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 1, 2],
        "end": [2, 3, 4],
    }
)
result = (
    df.lazy()
    .with_columns(
        pi.overlap_count("start", "end").alias("overlaps"),
    )
    .collect()
)

print(result["overlaps"].to_list())  # [1, 2, 1]
```

Intervals include their start and exclude their end, so touching intervals do not overlap.

### Count contained intervals

```python
df = pl.DataFrame({"start": [0, 2, 4], "end": [10, 5, 12]})
df.with_columns(pi.containment_count("start", "end").alias("contained"))
# contained: [1, 0, 0]
```

A contains B when `A.start <= B.start` and `B.end <= A.end`, excluding
the same row. Duplicates count one another. Empty intervals follow these
same endpoint inequalities: `[0, 5)` contains `[5, 5)`.

### Measure nesting depth

```python
df = pl.DataFrame({"start": [0, 2, 2, 3], "end": [10, 8, 8, 7]})
df.with_columns(pi.nesting_depth("start", "end").alias("depth"))
# depth: [0, 1, 1, 2]
```

Return the length of the longest strict containment chain above each interval.
Outermost intervals have depth zero; identical intervals never add a level.
See [strict endpoint semantics and grouped examples](https://jplauri.github.io/polars-intervals/usage/#nesting-depth).

### Maximize coverage with a budget

```python
df = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
selected = df.filter(pi.max_k_coverage("start", "end", k=2))
# Selects [-5, 4) and [6, 15): union measure 18.
```

Select at most `k` intervals whose union has maximum total measure. Among
maximum-coverage solutions, use the fewest intervals. The result is an exact,
deterministic Boolean mask in original row order; empty intervals are never
selected. See the [benchmarks and linked design notes](https://jplauri.github.io/polars-intervals/coverage-benchmarks/).

### Select a minimum-cost dominating set

```python
df = pl.DataFrame({"start": [0, 3, 6], "end": [4, 7, 10], "price": [1, 10, 1]})
df.filter(pi.minimum_cost_dominating_set("start", "end"))  # Middle row, unit costs.
df.filter(pi.minimum_cost_dominating_set("start", "end", cost="price"))  # Outer two.
```

Every row must be selected or overlap a selected row. Minimize total cost,
then selected count. Empty intervals are isolated and **all** must be selected,
even at zero cost. Omitted costs are units; an existing `cost` column has no
effect. See [semantics and grouped examples](https://jplauri.github.io/polars-intervals/usage/#select-a-minimum-cost-dominating-set).

### Select a maximum-weight clique

```python
df = pl.DataFrame({"start": [0, 1, 2, 10], "end": [5, 4, 3, 11], "value": [1, 1, 1, 9]})
df.filter(pi.max_weight_clique("start", "end"))  # First three rows: maximum cardinality.
df.filter(pi.max_weight_clique("start", "end", weight="value"))  # Last row: weight 9.
```

Select one globally maximum-weight set of pairwise intersecting intervals.
Omitted or `None` weights mean one per row, even if a column named `weight`
exists. Touching intervals do not intersect; empty intervals are isolated
singletons. Nonpositive explicit weights are omitted. See the
[contract and grouped examples](https://jplauri.github.io/polars-intervals/usage/#select-a-maximum-weight-clique)
and [benchmark comparison](https://jplauri.github.io/polars-intervals/clique-benchmarks/).

### Cover a target

Select the fewest intervals needed to cover `[0, 10)`:

```python
import polars as pl
import polars_intervals as pi

df = pl.DataFrame(
    {
        "start": [0, 0, 4, 6, 7],
        "end": [4, 6, 7, 10, 10],
    }
)
selected = df.filter(
    pi.minimum_cover(
        "start",
        "end",
        target_start=0,
        target_end=10,
    )
)

print(selected.rows())  # [(0, 6), (6, 10)]
```

## Contributing

See the [contributing guide](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md)
for source builds and development checks. Licensed under [MIT](https://github.com/jplauri/polars-intervals/blob/master/LICENSE).
