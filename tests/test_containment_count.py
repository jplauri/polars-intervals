"""Exercise the installed native plugin, never a Python implementation."""

from datetime import UTC, date, datetime

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_series_equal

from .dtypes import ENDPOINT_DTYPES


@pytest.mark.parametrize("context", ["select", "with_columns", "lazy_select", "lazy_with_columns"])
@pytest.mark.parametrize(
    "starts,ends,expected",
    [
        pytest.param([], [], [], id="empty_input"),
        pytest.param([1], [5], [0], id="single_interval"),
        pytest.param([0, 2, 4], [1, 3, 5], [0, 0, 0], id="disjoint"),
        pytest.param([0, 2, 3], [10, 8, 5], [2, 1, 0], id="nesting"),
        pytest.param([0, 3], [5, 8], [0, 0], id="crossing_not_containment"),
        pytest.param([1, 1, 1], [10, 8, 5], [2, 1, 0], id="equal_starts"),
        pytest.param([0, 2, 5], [10, 10, 10], [2, 1, 0], id="equal_ends"),
        pytest.param([1] * 5, [5] * 5, [4] * 5, id="duplicates"),
        pytest.param([1, 1, 1], [5, 5, 10], [1, 1, 2], id="full_equal_start_group"),
        pytest.param([0, 2, 0, 3, 2], [10, 8, 10, 5, 8], [4, 2, 4, 0, 2], id="nested_duplicates"),
        pytest.param([0, 0, 5, 10], [10, 0, 5, 10], [3, 0, 0, 0], id="empty_boundaries"),
        pytest.param([0, 5], [5, 5], [1, 0], id="empty_at_excluded_right_endpoint"),
        pytest.param([3, 3], [3, 3], [1, 1], id="duplicate_empties"),
        pytest.param([2, 3, 4], [2, 3, 4], [0, 0, 0], id="distinct_empties"),
        pytest.param([3, 2, 4, 3, 3, 2], [3, 2, 4, 3, 4, 3], [1, 0, 0, 1, 3, 3], id="empty_outer"),
        pytest.param([0, 5], [5, 10], [0, 0], id="touching_nonempty"),
        pytest.param([2, 0, 3, 1], [8, 10, 7, 9], [1, 3, 0, 2], id="shuffled_row_order"),
    ],
)
def test_named_semantics(context, starts, ends, expected):
    frame = pl.DataFrame(
        {"start": starts, "end": ends}, schema={"start": pl.Int64, "end": pl.Int64}
    )
    expression = pi.containment_count("start", "end").alias("count")
    lazy = context.startswith("lazy_")
    data = frame.lazy() if lazy else frame
    result = getattr(data, context.removeprefix("lazy_"))(expression)
    if lazy:
        assert result.collect_schema()["count"] == pl.UInt64
        result = result.collect()
    assert_series_equal(result["count"], pl.Series("count", expected, dtype=pl.UInt64))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_supported_dtypes_including_empty_input(dtype):
    frame = pl.DataFrame({"start": [0, 1, 1, 5], "end": [10, 5, 5, 5]}).cast(dtype)
    for data, expected in [(frame, [3, 2, 2, 0]), (frame.head(0), [])]:
        result = data.lazy().select(pi.containment_count("start", "end").alias("count")).collect()
        assert_series_equal(result["count"], pl.Series("count", expected, dtype=pl.UInt64))


def test_actual_dates_and_datetimes():
    for values in [
        [date(2025, 1, d) for d in [1, 2, 3, 10]],
        [datetime(2025, 1, d) for d in [1, 2, 3, 10]],  # noqa: DTZ001 -- test naive Datetime
        [datetime(2025, 1, d, tzinfo=UTC) for d in [1, 2, 3, 10]],
    ]:
        a, b, c, d = values
        frame = pl.DataFrame({"start": [a, b, c, d], "end": [d, c, c, d]})
        assert frame.select(pi.containment_count("start", "end")).to_series().to_list() == [
            3,
            1,
            0,
            0,
        ]


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_different_chunk_boundaries_and_slices(engine):
    starts = pl.concat([pl.Series([99, 2, 0]), pl.Series([3, 1, 99])], rechunk=False).slice(1, 4)
    ends = pl.concat(
        [pl.Series([99, 8]), pl.Series([10]), pl.Series([7, 9, 99])], rechunk=False
    ).slice(1, 4)
    assert starts.n_chunks() != ends.n_chunks()
    frame = pl.DataFrame({"start": starts, "end": ends})
    result = frame.lazy().select(pi.containment_count("start", "end")).collect(engine=engine)
    assert result.to_series().to_list() == [1, 3, 0, 2]


@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("ns", "UTC")])
def test_windows_and_group_aggregation(dtype):
    frame = pl.DataFrame(
        {"group": ["a", "b", "a", "b"], "start": [0, 1, 2, 1], "end": [10, 5, 5, 5]}
    ).with_columns(pl.col("start", "end").cast(dtype))
    expr = pi.containment_count("start", "end").alias("count")
    assert frame.lazy().select(expr.over("group")).collect()["count"].to_list() == [1, 1, 0, 1]
    assert frame.lazy().group_by("group", maintain_order=True).agg(expr).collect()[
        "count"
    ].to_list() == [[1, 0], [1, 1]]


def test_expression_arguments_and_filter_after_counting():
    frame = pl.DataFrame({"start": [2, 0, 3, 1], "end": [8, 10, 7, 9]})
    expression = pi.containment_count(pl.col("start") + 10, pl.col("end") + 10).alias("count")
    # Removing the innermost interval must not reduce already computed counts.
    result = frame.lazy().with_columns(expression).filter(pl.col("start") < 3).head(2).collect()
    assert result["count"].to_list() == [1, 3]
    assert frame.select(pi.containment_count(pl.col("start"), "end")).to_series().to_list() == [
        1,
        3,
        0,
        2,
    ]


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_extreme_endpoints(dtype):
    low, high = (-(2**63), 2**63 - 1) if dtype == pl.Int64 else (0, 2**64 - 1)
    frame = pl.DataFrame(
        {"start": [low, high - 1, high], "end": [high] * 3}, schema={"start": dtype, "end": dtype}
    )
    assert frame.select(pi.containment_count("start", "end")).to_series().to_list() == [2, 1, 0]
