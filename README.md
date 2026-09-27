# polars-intervals

Interval algorithms for Polars, with support for eager and lazy queries.

[Usage](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md) ·
[Benchmarks](https://github.com/jplauri/polars-intervals/blob/master/docs/benchmarks.md)

## Install

Requires Python 3.12+.

```sh
pip install polars-intervals
```

Or with uv: `uv add polars-intervals`.

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

### Maximize coverage with a budget

```python
df = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
selected = df.filter(pi.max_k_coverage("start", "end", k=2))
# Selects [-5, 4) and [6, 15): union measure 18.
```

Select at most `k` intervals whose union has maximum total measure. Among
maximum-coverage solutions, use the fewest intervals. The result is an exact,
deterministic Boolean mask in original row order; empty intervals are never
selected. See the [design and benchmarks](docs/coverage-benchmarks.md).

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

## Algorithms

| Function | Description |
| --- | --- |
| [`overlap_count`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#count-overlaps) | Count the other intervals overlapping each interval. |
| [`containment_count`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#count-containment) | Count the other intervals contained by each interval. |
| [`assign_lanes`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#assign-the-minimum-number-of-lanes) | Assign the fewest lanes with no overlaps within a lane. |
| [`max_weight_non_overlapping`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#select-a-globally-maximum-weight-schedule) | Select a non-overlapping subset with maximum total weight. |
| [`max_weight_with_capacity`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#select-with-a-simultaneous-capacity) | Maximize total weight under a simultaneous overlap limit. |
| [`minimum_cover`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#cover-one-continuous-target) | Cover a target with the fewest intervals. |
| [`minimum_cost_cover`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#cover-at-minimum-cost) | Cover a target at minimum total cost. |
| [`minimum_stabbing_points`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#minimum-stabbing-points) | Find the fewest points that hit every interval. |

## Contributing

See the [contributing guide](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md)
for source builds and development checks. Licensed under [MIT](https://github.com/jplauri/polars-intervals/blob/master/LICENSE).
