"""Exercise the installed native plugin, never a Python implementation."""

from datetime import UTC, date, datetime

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_series_equal


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


@pytest.mark.parametrize(
    "dtype",
    [pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64, pl.Date]
    + [
        pl.Datetime(unit, zone)
        for unit in ["ms", "us", "ns"]
        for zone in [None, "UTC", "Europe/Helsinki"]
    ],
    ids=str,
)
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


@pytest.mark.parametrize("column", ["start", "end", "both"])
@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("us", "UTC")])
def test_null_endpoints_are_rejected(column, dtype):
    frame = pl.DataFrame({"start": [0, 1], "end": [2, 3]}).cast(dtype)
    names = ["start", "end"] if column == "both" else [column]
    frame = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in names)
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        frame.select(pi.containment_count("start", "end"))


@pytest.mark.parametrize(
    "start_dtype,end_dtype",
    [
        (pl.Int32, pl.Int64),
        (pl.UInt64, pl.Int64),
        (pl.Date, pl.Int32),
        (pl.Datetime("ms"), pl.Datetime("us")),
        (pl.Datetime("us"), pl.Datetime("ns")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us", "Europe/Helsinki")),
    ],
)
def test_logical_dtypes_must_match_exactly(start_dtype, end_dtype):
    frame = pl.DataFrame(
        {"start": pl.Series([0], dtype=start_dtype), "end": pl.Series([1], dtype=end_dtype)}
    )
    with pytest.raises(pl.exceptions.PolarsError, match="matching.*dtypes"):
        frame.select(pi.containment_count("start", "end"))


@pytest.mark.parametrize(
    "dtype", [pl.Float64, pl.Boolean, pl.String, pl.Duration("us"), pl.Time, pl.Int128]
)
def test_unsupported_dtypes_even_when_empty(dtype):
    frame = pl.DataFrame(schema={"start": dtype, "end": dtype})
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype"):
        frame.select(pi.containment_count("start", "end"))


def test_invalid_interval_reports_first_original_index():
    frame = pl.DataFrame({"start": [0, 3, 4], "end": [0, 2, 1]})
    with pytest.raises(pl.exceptions.ComputeError, match="index 1"):
        frame.select(pi.containment_count("start", "end"))


@pytest.mark.parametrize("start", [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)])
def test_lengths_must_match_without_broadcasting(start):
    frame = pl.DataFrame({"start": [0, 1], "end": [3, 4]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.containment_count(start, "end"))
