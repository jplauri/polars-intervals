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
| [`overlap_count`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#count-within-groups) | Count the other intervals overlapping each interval. |
| [`assign_lanes`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#assign-the-minimum-number-of-lanes) | Assign the fewest lanes with no overlaps within a lane. |
| [`max_weight_non_overlapping`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#select-a-globally-maximum-weight-schedule) | Select a non-overlapping subset with maximum total weight. |
| [`max_weight_with_capacity`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#select-with-a-simultaneous-capacity) | Maximize total weight under a simultaneous overlap limit. |
| [`minimum_cover`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#cover-one-continuous-target) | Cover a target with the fewest intervals. |
| [`minimum_cost_cover`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#cover-one-continuous-target) | Cover a target at minimum total cost. |
| [`minimum_stabbing_points`](https://github.com/jplauri/polars-intervals/blob/master/docs/usage.md#minimum-stabbing-points) | Find the fewest points that hit every interval. |

## Contributing

See the [contributing guide](https://github.com/jplauri/polars-intervals/blob/master/CONTRIBUTING.md)
for source builds and development checks. Licensed under [MIT](https://github.com/jplauri/polars-intervals/blob/master/LICENSE).
