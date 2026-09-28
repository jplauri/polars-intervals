"""Exercise the installed native scalar-list plugin, with exact physical ticks."""

from datetime import date
from itertools import combinations

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES


def frame(starts, ends, dtype=pl.Int64):
    return pl.DataFrame({"start": starts, "end": ends}, schema={"start": dtype, "end": dtype})


def points_expr():
    return pi.minimum_stabbing_points("start", "end").alias("points")


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES)
def test_select_lazy_chunks_shuffled_and_empty(dtype):
    df = frame([5, 0, 2], [9, 4, 6], dtype)
    expected = pl.DataFrame({"points": [pl.Series([3, 8]).cast(dtype)]})
    expr = points_expr()
    for source in (df, pl.concat([df.head(1), df.tail(2)], rechunk=False)):
        assert_frame_equal(source.select(expr), expected)
        assert_frame_equal(source.lazy().select(expr).collect(), expected)
        assert source.lazy().select(expr).collect_schema()["points"] == pl.List(dtype)
        empty = source.clear().select(expr)
        assert empty.shape == (1, 1)
        assert empty.schema["points"] == pl.List(dtype)
        assert empty.to_series().to_list() == [[]]
        assert_frame_equal(source.clear().lazy().select(expr).collect(), empty)


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES)
def test_group_aggregation_and_window(dtype):
    df = frame([0, 0, 2, 2, 5], [4, 2, 6, 4, 9], dtype).with_columns(
        pl.Series("group", ["a", "b", "a", "b", "a"])
    )
    expected = pl.DataFrame(
        {
            "group": ["a", "b"],
            "points": [pl.Series([3, 8]).cast(dtype), pl.Series([1, 3]).cast(dtype)],
        }
    )
    assert_frame_equal(df.group_by("group").agg(points_expr()).sort("group"), expected)
    assert_frame_equal(
        df.lazy().group_by("group").agg(points_expr()).sort("group").collect(), expected
    )
    window = df.select(points_expr().over("group"))
    assert window.schema["points"] == pl.List(dtype)
    assert_frame_equal(window, expected[[0, 1, 0, 1, 0]].select("points"))
    empty = df.clear().group_by("group").agg(points_expr())
    assert empty.height == 0
    assert empty.schema["points"] == pl.List(dtype)


def test_date_example():
    df = frame([date(2026, 1, 1)], [date(2026, 1, 5)], pl.Date)
    assert df.select(points_expr()).to_series().to_list() == [[date(2026, 1, 4)]]


@pytest.mark.parametrize("unit", ["ms", "us", "ns"])
@pytest.mark.parametrize("zone", [None, "UTC", "Europe/Helsinki"])
def test_datetime_predecessor_is_exactly_one_physical_tick(unit, zone):
    dtype = pl.Datetime(unit, zone)
    # Includes negative and positive ticks and a touching boundary.
    df = frame([-5, 1000, 1005], [-1, 1005, 1010], dtype)
    result = df.select(points_expr()).to_series()
    assert result.dtype == pl.List(dtype)
    assert result.item().to_physical().to_list() == [-2, 1004, 1009]


@pytest.mark.parametrize(
    ("dtype", "starts", "ends", "expected"),
    [
        (pl.Int64, [-(2**63), 2**63 - 2], [-(2**63) + 1, 2**63 - 1], [-(2**63), 2**63 - 2]),
        (pl.UInt64, [2**64 - 2, 0], [2**64 - 1, 1], [0, 2**64 - 2]),
    ],
)
def test_extreme_endpoints(dtype, starts, ends, expected):
    result = frame(starts, ends, dtype).select(points_expr()).to_series()
    assert result.dtype == pl.List(dtype)
    assert result.to_list() == [expected]


@pytest.mark.parametrize("position", [0, 1, 2, 3])
def test_empty_interval_original_index(position):
    rows = [(6, 8), (0, 2), (2, 5)]
    rows.insert(position, (3, 3))
    starts, ends = zip(*rows)
    df = frame(starts, ends)
    with pytest.raises(
        pl.exceptions.ComputeError, match=f"cannot stab empty interval at index {position}"
    ):
        df.lazy().select(points_expr()).collect()


def test_empty_interval_alone_and_group_error():
    with pytest.raises(pl.exceptions.ComputeError, match="cannot stab empty interval at index 0"):
        frame([3], [3]).select(points_expr())
    with pytest.raises(pl.exceptions.ComputeError, match="cannot stab empty interval at index 1"):
        frame([0, 3], [2, 3]).with_columns(pl.lit("a").alias("g")).group_by("g").agg(points_expr())


def test_expression_arguments_and_unequal_lengths():
    df = frame([0, 2, 5], [4, 6, 9])
    assert df.select(
        pi.minimum_stabbing_points(pl.col("start"), pl.col("end"))
    ).to_series().to_list() == [[3, 8]]
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        df.select(pi.minimum_stabbing_points(pl.col("start").head(1), "end"))


def test_small_plugin_instances_against_all_coordinate_subsets():
    universe = [(s, e) for s in range(-2, 3) for e in range(s + 1, 4)]
    coordinates = range(-2, 4)
    for rows in combinations(universe, 3):
        s, e = zip(*rows)
        result = frame(s, e).select(points_expr()).to_series().item().to_list()
        optimum = next(
            k
            for k in range(4)
            if any(
                all(any(s <= p < e for p in pts) for s, e in rows)
                for pts in combinations(coordinates, k)
            )
        )
        assert len(result) == optimum
        assert result == sorted(set(result))
        assert all(any(s <= p < e for p in result) for s, e in rows)
