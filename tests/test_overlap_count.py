"""Test the installed Rust plugin from the repository root:

    python -m pip install ".[test]"
    python -I -m pytest

Use a Python 3.12+ virtual environment with a Rust toolchain available.
"""

import polars as pl
import pytest
from polars.testing import assert_frame_equal, assert_series_equal

import polars_intervals as pi


@pytest.mark.parametrize("context", ["select", "with_columns"])
@pytest.mark.parametrize(
    "starts, ends, counts",
    [
        pytest.param([], [], [], id="empty_input"),
        pytest.param([1], [4], [0], id="single_interval"),
        pytest.param([5, 1, 3, 2], [7, 4, 6, 3], [1, 2, 2, 1], id="basic_overlaps"),
        pytest.param([8, 1, 4], [9, 2, 6], [0, 0, 0], id="disjoint"),
        pytest.param([3, 1, 5], [5, 3, 7], [0, 0, 0], id="touching"),
        pytest.param([2, 0, 1, 6], [3, 10, 5, 9], [2, 3, 2, 1], id="nested"),
        pytest.param([1, 1, 1], [4, 4, 4], [2, 2, 2], id="duplicates"),
        pytest.param([1, 1, 4], [1, 1, 4], [0, 0, 0], id="empty_intervals"),
        pytest.param(
            [2, 0, 4, 0, 1],
            [2, 4, 4, 0, 3],
            [0, 1, 0, 0, 1],
            id="empty_intervals_inside_and_at_boundaries",
        ),
    ],
)
def test_interval_semantics_in_select_and_with_columns(context, starts, ends, counts):
    frame = pl.DataFrame(
        {"start": starts, "end": ends},
        schema={"start": pl.Int64, "end": pl.Int64},
    ).with_row_index("row")
    expected = frame.with_columns(pl.Series("count", counts, dtype=pl.UInt64))
    expression = pi.overlap_count("start", "end").alias("count")

    if context == "select":
        result = frame.select("row", expression)
        expected = expected.select("row", "count")
    else:
        result = frame.with_columns(expression)
    assert_frame_equal(result, expected)


@pytest.mark.parametrize("context", ["select", "with_columns"])
def test_invalid_interval_reports_original_row_in_select_and_with_columns(context):
    frame = pl.DataFrame({"start": [2, 0, 5, 6], "end": [2, 1, 4, 8]})
    expression = pi.overlap_count("start", "end")

    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        if context == "select":
            frame.select(expression)
        else:
            frame.with_columns(expression)


def test_column_names_return_an_expression_with_half_open_counts():
    frame = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
    expression = pi.overlap_count("start", "end")
    assert isinstance(expression, pl.Expr)
    result = frame.lazy().select(expression.alias("count")).collect()
    assert_series_equal(
        result["count"], pl.Series("count", [1, 1, 2, 0], dtype=pl.UInt64)
    )


@pytest.mark.parametrize(
    "start, end",
    [
        pytest.param(pl.col("start"), "end", id="expression_start"),
        pytest.param("start", pl.col("end"), id="expression_end"),
        pytest.param(pl.col("start") + 10, pl.col("end") + 10, id="shifted_expressions"),
    ],
)
def test_expressions_and_mixed_arguments_preserve_row_order(start, end):
    frame = pl.DataFrame({"start": [4, 1, 3, 2, 2], "end": [8, 3, 5, 4, 2]})
    result = (
        frame.lazy()
        .with_columns(pi.overlap_count(start, end).alias("count"))
        .collect()
    )
    assert result["start"].to_list() == [4, 1, 3, 2, 2]
    assert_series_equal(
        result["count"], pl.Series("count", [1, 1, 2, 2, 0], dtype=pl.UInt64)
    )


@pytest.mark.parametrize(
    "dtype",
    [pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64],
    ids=str,
)
def test_all_supported_integer_dtypes(dtype):
    frame = pl.DataFrame(
        {"start": [1, 2, 2, 3, 6], "end": [9, 5, 5, 3, 7]},
        schema={"start": dtype, "end": dtype},
    )
    result = (
        frame.lazy()
        .select(pi.overlap_count("start", "end").alias("count"))
        .collect()
    )
    assert_series_equal(
        result["count"], pl.Series("count", [3, 2, 2, 0, 1], dtype=pl.UInt64)
    )


@pytest.mark.parametrize(
    "starts, ends, counts",
    [
        pytest.param([], [], [], id="empty_input"),
        pytest.param([1], [4], [0], id="single_interval"),
        pytest.param([2], [2], [0], id="empty_interval"),
        pytest.param([1, 3, 5], [3, 5, 8], [0, 0, 0], id="touching"),
    ],
)
def test_empty_single_and_touching_collections(starts, ends, counts):
    frame = pl.DataFrame(
        {"start": starts, "end": ends},
        schema={"start": pl.Int64, "end": pl.Int64},
    )
    query = frame.lazy().select(pi.overlap_count("start", "end").alias("count"))
    assert query.collect_schema()["count"] == pl.UInt64
    assert_series_equal(
        query.collect()["count"], pl.Series("count", counts, dtype=pl.UInt64)
    )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_counts_across_chunks_and_streaming_batches(engine):
    frame = pl.concat(
        [pl.DataFrame({"start": [1, 1], "end": [5, 5]}) for _ in range(4)],
        rechunk=False,
    )
    assert frame["start"].n_chunks() > 1
    query = frame.lazy().select(pi.overlap_count("start", "end").alias("count"))
    assert_series_equal(
        query.collect(engine=engine)["count"],
        pl.Series("count", [7] * 8, dtype=pl.UInt64),
    )


def test_filter_and_slice_after_counts_preserve_the_collection():
    frame = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
    result = (
        frame.lazy()
        .with_columns(pi.overlap_count("start", "end").alias("count"))
        .filter(pl.col("start") < 3)
        .head(2)
        .collect()
    )
    assert result["count"].to_list() == [1, 2]


def test_window_and_group_by_count_within_each_group():
    frame = pl.DataFrame(
        {
            "group": ["a", "b", "a", "b"],
            "start": [1, 1, 2, 5],
            "end": [4, 4, 3, 6],
        }
    )
    expression = pi.overlap_count("start", "end")
    window = frame.lazy().with_columns(expression.over("group").alias("count"))
    assert window.collect()["count"].to_list() == [1, 0, 1, 0]
    grouped = (
        frame.lazy()
        .group_by("group", maintain_order=True)
        .agg(expression.alias("count"))
        .collect()
    )
    assert grouped["count"].to_list() == [[1, 1], [0, 0]]


@pytest.mark.parametrize(
    "starts, ends, message",
    [
        pytest.param(
            pl.Series([1, None]), pl.Series([3, 4]), "null endpoints", id="null_start"
        ),
        pytest.param(
            pl.Series([1, 2]), pl.Series([3, None]), "null endpoints", id="null_end"
        ),
        pytest.param(
            pl.Series([None], dtype=pl.Int64),
            pl.Series([None], dtype=pl.Int64),
            "null endpoints",
            id="all_null",
        ),
        pytest.param(pl.Series([4]), pl.Series([3]), "index 0", id="reversed_interval"),
        pytest.param(
            pl.Series([1], dtype=pl.Int32),
            pl.Series([3], dtype=pl.Int64),
            "matching integer dtypes",
            id="mixed_dtypes",
        ),
        pytest.param(pl.Series([1.0]), pl.Series([3.0]), "integer dtype", id="floats"),
        pytest.param(pl.Series(["a"]), pl.Series(["b"]), "integer dtype", id="strings"),
    ],
)
def test_rejects_nulls_invalid_intervals_and_unsupported_dtypes(starts, ends, message):
    frame = pl.DataFrame({"start": starts, "end": ends})
    with pytest.raises(pl.exceptions.PolarsError, match=message):
        frame.lazy().select(pi.overlap_count("start", "end")).collect()


@pytest.mark.parametrize(
    "start",
    [
        pytest.param(pl.col("start").head(1), id="short_column"),
        pytest.param(pl.lit(1, dtype=pl.Int64), id="scalar"),
    ],
)
def test_rejects_unequal_expression_lengths_without_broadcasting(start):
    frame = pl.DataFrame({"start": [1, 2], "end": [3, 4]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.lazy().select(pi.overlap_count(start, "end")).collect()
