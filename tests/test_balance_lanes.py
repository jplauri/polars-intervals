"""Balancing contracts at the Polars expression boundary."""

from collections import Counter
from datetime import UTC, date, datetime
from pathlib import Path
from random import Random

import polars as pl
import polars_intervals as pi
import pytest
from polars.plugins import register_plugin_function

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES
from .test_assign_lanes import assert_coloring


def score(lanes):
    counts = list(Counter(lanes.to_list()).values())
    return (max(counts) - min(counts), sum(x * x for x in counts)) if counts else (0, 0)


def expression(method, *, max_work=100_000, start="start", end="end", lanes="lane"):
    if method == "construct":
        return pi.assign_balanced_lanes(start, end, max_work=max_work)
    return pi.assign_balanced_lanes(start, end, initial_lanes=lanes, max_work=max_work)


def baseline(frame):
    return frame.with_columns(pi.assign_lanes("start", "end").alias("lane"))


@pytest.mark.parametrize("count", [1, 4])
def test_plugin_rejects_invalid_arity_during_schema_inference(count):
    expr = register_plugin_function(
        plugin_path=Path(pi.__file__).parent,
        function_name="assign_balanced_lanes_plugin",
        args=["start"] * count,
        kwargs={"max_work": "0"},
        is_elementwise=False,
    )
    with pytest.raises(pl.exceptions.PolarsError, match="requires two or three inputs"):
        pl.DataFrame({"start": [0]}).lazy().select(expr).collect_schema()


@pytest.mark.parametrize("max_work", [0, 100_000])
def test_explicit_none_constructs_without_reading_existing_lane_column(max_work):
    frame = pl.DataFrame({"start": [0, 0, 1, 1], "end": [3, 3, 1, 1], "lane": [None] * 4})
    explicit = frame.select(
        pi.assign_balanced_lanes("start", "end", initial_lanes=None, max_work=max_work)
    ).to_series()
    implicit = frame.select(pi.assign_balanced_lanes("start", "end", max_work=max_work)).to_series()
    assert explicit.equals(implicit)
    assert_coloring(frame, explicit)


@pytest.mark.parametrize("max_work", [0, 100_000])
def test_empty_column_name_is_a_supplied_assignment(max_work):
    frame = pl.DataFrame({"start": [0, 0, 1, 1], "end": [3, 3, 1, 1], "": [1, 0, 1, 1]})
    result = frame.select(
        pi.assign_balanced_lanes("start", "end", initial_lanes="", max_work=max_work)
    ).to_series()
    assert_coloring(frame, result)
    assert score(result) <= score(frame[""])
    if max_work == 0:
        assert result.to_list() == frame[""].to_list()


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("lazy", [False, True])
@pytest.mark.parametrize(
    "starts,ends",
    [
        ([], []),
        ([1], [3]),
        ([0, 1, 2], [1, 2, 3]),
        ([0, 1, 2, 3], [10, 9, 8, 7]),
        ([1] * 4, [5] * 4),
        ([0, 1, 2, 3, 4], [2, 3, 4, 5, 6]),
        ([0, 1, 2, 3, 20, 21], [10, 9, 8, 7, 23, 24]),
        ([5] * 10, [5] * 10),
        ([0, 0, 2, 4, 1], [4, 0, 2, 4, 3]),
        ([0, 0, 1, 1], [3, 3, 1, 1]),
    ],
)
def test_semantics_nonregression_and_determinism(starts, ends, method, lazy):
    frame = baseline(
        pl.DataFrame({"start": starts, "end": ends}, schema={"start": pl.Int64, "end": pl.Int64})
    )
    expr = expression(method).alias("balanced")
    assert isinstance(expr, pl.Expr)
    query = frame.lazy().select(expr) if lazy else frame.select(expr)
    lanes = (query.collect() if lazy else query).to_series()
    assert_coloring(frame, lanes)
    assert score(lanes) <= score(frame["lane"])
    assert lanes.to_list() == frame.select(expr).to_series().to_list()
    zero = frame.select(expression(method, max_work=0)).to_series()
    assert zero.to_list() == frame["lane"].to_list()


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_supported_endpoints_unequal_chunks_slices_and_expressions(method, dtype, engine):
    intervals = [(i // 4, i // 4 + i % 5) for i in range(80)]
    Random(42).shuffle(intervals)
    starts, ends = zip(*intervals, strict=True)
    frame = pl.DataFrame({"start": starts, "end": ends}).with_columns(pl.all().cast(dtype))
    start = pl.concat([frame["start"].head(23), frame["start"].slice(23)], rechunk=False)
    end = pl.concat(
        [frame["end"].head(11), frame["end"].slice(11, 42), frame["end"].slice(53)],
        rechunk=False,
    )
    frame = pl.DataFrame([start, end])
    assert frame["start"].n_chunks() == 2
    assert frame["end"].n_chunks() == 3
    expr = expression(method, start=pl.col("start"), lanes=pl.col("lane")).alias("balanced")
    for subset in [frame, frame.slice(3, 60), frame.head(0)]:
        subset = baseline(subset)
        # Lane chunks differ from both endpoint columns as well.
        lane = subset["lane"]
        subset = subset.with_columns(pl.concat([lane.head(5), lane.slice(5)], rechunk=False))
        query = subset.lazy().select(expr)
        assert query.collect_schema()["balanced"] == pl.UInt32
        lanes = query.collect(engine=engine).to_series()
        assert_coloring(subset, lanes)
        assert score(lanes) <= score(subset["lane"])
        assert lanes.to_list() == subset.rechunk().select(expr).to_series().to_list()
        assert (
            lanes.to_list()
            == baseline(subset.with_columns(pl.col("start", "end").to_physical().cast(pl.Int64)))
            .select(expr)
            .to_series()
            .to_list()
        )


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_window_and_grouped_list_aggregation(method, dtype, engine):
    frame = pl.DataFrame({"start": [0, 20, 0, 22, 1, 24, 1], "end": [10, 23, 9, 25, 1, 26, 1]})
    frame = frame.with_columns(
        pl.all().cast(dtype), pl.Series("group", ["a", "b", "a", "b", "a", "b", "a"])
    ).with_columns(pi.assign_lanes("start", "end").over("group").alias("lane"))
    expr = expression(method).alias("balanced")
    result = frame.lazy().with_columns(expr.over("group")).collect(engine=engine)
    grouped = frame.lazy().group_by("group", maintain_order=True).agg(expr).collect(engine=engine)
    assert grouped.schema["balanced"] == pl.List(pl.UInt32)
    for group, labels in grouped.iter_rows():
        subset = result.filter(pl.col("group") == group)
        assert_coloring(subset, subset["balanced"])
        assert score(subset["balanced"]) <= score(subset["lane"])
        assert labels == subset["balanced"].to_list()
        assert labels == subset.select(expr).to_series().to_list()


@pytest.mark.parametrize("dtype", INTEGER_DTYPES, ids=str)
def test_lane_integer_widths(dtype):
    frame = pl.DataFrame({"start": [0, 0, 1, 1], "end": [3, 3, 1, 1], "lane": [0, 1, 0, 0]})
    frame = frame.with_columns(pl.col("lane").cast(dtype))
    lanes = frame.select(pi.assign_balanced_lanes("start", "end", initial_lanes="lane")).to_series()
    assert_coloring(frame, lanes)
    assert score(lanes) == (0, 8)


@pytest.mark.parametrize("method", ["construct", "repair"])
def test_empty_rows_can_move_and_old_contract_stays_unchanged(method):
    frame = baseline(pl.DataFrame({"start": [0, 0, 1, 1], "end": [3, 3, 1, 1]}))
    assert frame["lane"].to_list()[2:] == [0, 0]
    lanes = frame.select(expression(method)).to_series()
    assert_coloring(frame, lanes)
    assert score(frame["lane"]) == (2, 10)
    assert score(lanes) == (0, 8)
    assert sorted(lanes.to_list()[2:]) == [0, 1]


def test_simultaneous_component_flips_reach_two_lane_optimum():
    starts, ends, lanes = [], [], []
    for component, leaves in enumerate([9, 8, 7, 6]):
        offset = component * 30
        large_side = 0 if component < 2 else 1
        starts.append(offset)
        ends.append(offset + 2 * leaves + 1)
        lanes.append(1 - large_side)
        for leaf in range(leaves):
            starts.append(offset + 2 * leaf + 1)
            ends.append(offset + 2 * leaf + 2)
            lanes.append(large_side)
    frame = pl.DataFrame({"start": starts, "end": ends, "lane": lanes})
    assert score(frame["lane"]) == (4, 586)
    result = frame.select(
        pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=1_000_000)
    ).to_series()
    assert_coloring(frame, result)
    assert score(result) == (0, 578)


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("budget", [0, 1, 100_000, 2**63, 2**64 - 1])
def test_full_uint64_option_range(method, budget):
    frame = pl.DataFrame({"start": [0, 0], "end": [2, 2], "lane": [1, 0]})
    result = frame.select(expression(method, max_work=budget)).to_series()
    assert_coloring(frame, result)
    if budget == 0 and method == "repair":
        assert result.to_list() == [1, 0]


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("value", [True, False, 1.0, "1", None, pl.lit(1)])
def test_noninteger_options(method, value):
    with pytest.raises(TypeError, match="max_work.*integer"):
        expression(method, max_work=value)


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("value", [-1, 2**64])
def test_out_of_range_options(method, value):
    with pytest.raises(ValueError, match="max_work.*UInt64"):
        expression(method, max_work=value)


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Boolean,
        pl.Float32,
        pl.Float64,
        pl.String,
        pl.Null,
        pl.Date,
        pl.Datetime("us"),
        pl.Duration("ns"),
        pl.Time,
        pl.Int128,
        pl.UInt128,
        pl.Decimal(20, 0),
        pl.Categorical,
        pl.Enum(["a"]),
        pl.Object,
        pl.List(pl.Int64),
        pl.Array(pl.Int64, 2),
        pl.Struct({"x": pl.Int64}),
    ],
)
@pytest.mark.parametrize("max_work", [0, 100_000])
def test_unsupported_lane_dtypes_are_safe_on_empty_input(dtype, max_work):
    for length in [0, 1]:
        frame = pl.DataFrame(
            {
                "start": pl.Series([0] * length, dtype=pl.Int64),
                "end": pl.Series([0] * length, dtype=pl.Int64),
                "lane": pl.Series([None] * length, dtype=dtype),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError):
            frame.select(
                pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=max_work)
            )


@pytest.mark.parametrize("max_work", [0, 100_000])
@pytest.mark.parametrize(
    "values,dtype,pattern",
    [
        ([0, None], pl.Int64, "null lane"),
        ([0, -1], pl.Int64, "UInt32-range.*index 1"),
        ([0, 2**32], pl.UInt64, "UInt32-range.*index 1"),
        ([0, 2**64 - 1], pl.UInt64, "UInt32-range.*index 1"),
        ([0, 2**32 - 1], pl.UInt64, "lane"),
        ([0, 2], pl.UInt32, "lane"),
        ([1, 2], pl.UInt32, "lane"),
        ([0, 0], pl.UInt32, "lane"),
    ],
)
def test_invalid_lane_values_sparse_palettes_and_conflicts(values, dtype, pattern, max_work):
    frame = pl.DataFrame({"start": [0, 0], "end": [2, 2], "lane": pl.Series(values, dtype=dtype)})
    with pytest.raises(pl.exceptions.PolarsError, match=pattern):
        frame.select(
            pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=max_work)
        )


@pytest.mark.parametrize("max_work", [0, 100_000])
def test_proper_but_nonminimum_even_when_equitable(max_work):
    for starts, ends in [([0, 1], [1, 2]), ([1, 1], [1, 1])]:
        frame = pl.DataFrame({"start": starts, "end": ends, "lane": [0, 1]})
        with pytest.raises(pl.exceptions.PolarsError, match="minimum"):
            frame.select(
                pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=max_work)
            )


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("start", [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)])
def test_no_endpoint_broadcasting(method, start):
    frame = pl.DataFrame({"start": [0, 0], "end": [2, 2], "lane": [0, 1]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(expression(method, start=start, max_work=0))


@pytest.mark.parametrize("lanes", [pl.col("lane").head(1), pl.lit(0, dtype=pl.UInt32)])
def test_no_lane_broadcasting(lanes):
    frame = pl.DataFrame({"start": [0, 0], "end": [2, 2], "lane": [0, 1]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.assign_balanced_lanes("start", "end", initial_lanes=lanes, max_work=0))


@pytest.mark.parametrize("method", ["construct", "repair"])
def test_missing_columns(method):
    frame = pl.DataFrame({"start": [0], "end": [1], "lane": [0]})
    for column in ["start", "end"] + (["lane"] if method == "repair" else []):
        with pytest.raises(pl.exceptions.ColumnNotFoundError):
            frame.drop(column).lazy().select(expression(method)).collect()


@pytest.mark.parametrize("method", ["construct", "repair"])
def test_real_dates_and_datetimes_across_daylight_saving_change(method):
    for dtype, starts, ends in [
        (pl.Date, [date(2026, 1, d) for d in [1, 2, 3]], [date(2026, 1, d) for d in [3, 4, 5]]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in [(0, 30), (1, 0), (0, 45)]],
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in [(1, 0), (1, 30), (1, 15)]],
        ),
    ]:
        frame = baseline(
            pl.DataFrame({"start": starts, "end": ends}, schema={"start": dtype, "end": dtype})
        )
        result = frame.select(expression(method)).to_series()
        assert_coloring(frame, result)
        assert score(result) <= score(frame["lane"])


@pytest.mark.parametrize("method", ["construct", "repair"])
@pytest.mark.parametrize("dtype", [pl.UInt64, pl.Int64, pl.Datetime("ns")])
def test_extreme_values_and_single_ticks(method, dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    minimum = 0 if dtype == pl.UInt64 else -(2**63)
    frame = baseline(
        pl.DataFrame(
            {"start": [minimum, maximum - 2, maximum - 1, maximum], "end": [maximum] * 4},
            schema={"start": dtype, "end": dtype},
        )
    )
    result = frame.select(expression(method)).to_series()
    assert_coloring(frame, result)
    assert score(result) <= score(frame["lane"])
