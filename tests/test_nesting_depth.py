from datetime import UTC, date, datetime
from functools import cache
from random import Random

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
        pytest.param([0, 3, 6], [5, 8, 10], [0, 0, 0], id="partial_crossings"),
        pytest.param([0, 1, 2, 3], [10, 9, 8, 7], [0, 1, 2, 3], id="deep_strict_chain"),
        pytest.param([1, 1, 1], [10, 8, 5], [0, 1, 2], id="equal_starts"),
        pytest.param([0, 2, 5], [10, 10, 10], [0, 1, 2], id="equal_ends"),
        pytest.param([1, 1, 1], [5, 5, 5], [0, 0, 0], id="identical_duplicates"),
        pytest.param([0, 2, 2, 3], [10, 8, 8, 7], [0, 1, 1, 2], id="duplicates_inside_chain"),
        pytest.param([0, 0, 2], [10, 10, 8], [0, 0, 1], id="duplicate_outer_intervals"),
        pytest.param([1, 1, 1], [10, 10, 8], [0, 0, 1], id="same_start_duplicate_then_child"),
        pytest.param([0, 0, 2], [10, 10, 10], [0, 0, 1], id="same_end_duplicate_then_child"),
        pytest.param([0, 2, 3, 4], [8, 10, 9, 5], [0, 0, 1, 2], id="mixed_containment_crossing"),
        pytest.param([0, 1, 2, 4], [8, 9, 10, 5], [0, 0, 0, 1], id="depth_not_container_count"),
        pytest.param([0, 5], [5, 5], [0, 1], id="empty_at_excluded_right_endpoint"),
        pytest.param([3, 3], [3, 3], [0, 0], id="identical_empties"),
        pytest.param([2, 3, 4], [2, 3, 4], [0, 0, 0], id="distinct_empties"),
        pytest.param([0, 2, 5, 5], [10, 5, 5, 5], [0, 1, 2, 2], id="chain_ending_in_empties"),
        pytest.param([0, 5], [5, 10], [0, 0], id="touching_nonempty"),
        pytest.param([2, 0, 3, 1], [8, 10, 7, 9], [2, 0, 3, 1], id="original_row_order"),
    ],
)
def test_named_semantics(context, starts, ends, expected):
    frame = pl.DataFrame(
        {"start": starts, "end": ends}, schema={"start": pl.Int64, "end": pl.Int64}
    )
    expression = pi.nesting_depth("start", "end").alias("depth")
    lazy = context.startswith("lazy_")
    data = frame.lazy() if lazy else frame
    result = getattr(data, context.removeprefix("lazy_"))(expression)
    if lazy:
        assert result.collect_schema()["depth"] == pl.UInt64
        result = result.collect()
    assert result["depth"].null_count() == 0
    assert_series_equal(result["depth"], pl.Series("depth", expected, dtype=pl.UInt64))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_supported_dtypes_including_empty_input(dtype):
    frame = pl.DataFrame({"start": [0, 1, 1, 5], "end": [10, 5, 5, 5]}).cast(dtype)
    for data, expected in [(frame, [0, 1, 1, 2]), (frame.head(0), [])]:
        result = data.lazy().select(pi.nesting_depth("start", "end").alias("depth")).collect()
        assert_series_equal(result["depth"], pl.Series("depth", expected, dtype=pl.UInt64))
        assert result["depth"].null_count() == 0


def test_actual_dates_and_datetimes():
    for values in [
        [date(2025, 1, d) for d in [1, 2, 3, 10]],
        [datetime(2025, 1, d) for d in [1, 2, 3, 10]],  # noqa: DTZ001 -- test naive Datetime
        [datetime(2025, 1, d, tzinfo=UTC) for d in [1, 2, 3, 10]],
    ]:
        a, b, c, d = values
        frame = pl.DataFrame({"start": [a, b, c, d], "end": [d, c, c, d]})
        assert frame.select(pi.nesting_depth("start", "end")).to_series().to_list() == [0, 1, 2, 1]


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_different_chunk_boundaries_and_slices(engine):
    starts = pl.concat([pl.Series([99, 2, 0]), pl.Series([3, 1, 99])], rechunk=False).slice(1, 4)
    ends = pl.concat(
        [pl.Series([99, 8]), pl.Series([10]), pl.Series([7, 9, 99])], rechunk=False
    ).slice(1, 4)
    assert starts.n_chunks() != ends.n_chunks()
    frame = pl.DataFrame({"start": starts, "end": ends})
    result = frame.lazy().select(pi.nesting_depth("start", "end")).collect(engine=engine)
    assert result.to_series().to_list() == [2, 0, 3, 1]


@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("ns", "Europe/Helsinki")])
@pytest.mark.parametrize("lazy", [False, True])
def test_windows_and_group_aggregation(dtype, lazy):
    frame = pl.DataFrame(
        {"group": ["a", "b", "a", "b"], "start": [0, 1, 2, 1], "end": [10, 5, 5, 5]}
    ).with_columns(pl.col("start", "end").cast(dtype))
    data = frame.lazy() if lazy else frame
    expr = pi.nesting_depth("start", "end").alias("depth")
    window = data.with_columns(expr.over("group"))
    grouped = data.group_by("group", maintain_order=True).agg(expr)
    if lazy:
        window, grouped = window.collect(), grouped.collect()
    assert_series_equal(window["depth"], pl.Series("depth", [0, 0, 1, 0], dtype=pl.UInt64))
    assert grouped["depth"].dtype == pl.List(pl.UInt64)
    assert grouped["depth"].to_list() == [[0, 1], [0, 0]]


def test_expression_arguments_and_filter_after_depths():
    frame = pl.DataFrame({"start": [2, 0, 3, 1], "end": [8, 10, 7, 9]})
    expression = pi.nesting_depth(pl.col("start") + 10, pl.col("end") + 10).alias("depth")
    # Removing the outermost interval must not reduce already computed depths.
    result = frame.lazy().with_columns(expression).filter(pl.col("start") > 0).head(2).collect()
    assert result["depth"].to_list() == [2, 3]
    assert frame.select(pi.nesting_depth(pl.col("start"), "end")).to_series().to_list() == [
        2,
        0,
        3,
        1,
    ]


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_extreme_endpoints(dtype):
    low, high = (-(2**63), 2**63 - 1) if dtype == pl.Int64 else (0, 2**64 - 1)
    frame = pl.DataFrame(
        {"start": [low, high - 1, high], "end": [high] * 3}, schema={"start": dtype, "end": dtype}
    )
    assert frame.select(pi.nesting_depth("start", "end")).to_series().to_list() == [0, 1, 2]


@pytest.mark.parametrize("column", ["start", "end", "both"])
@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("us", "UTC")])
def test_null_endpoints_are_rejected(column, dtype):
    frame = pl.DataFrame({"start": [0, 1], "end": [2, 3]}).cast(dtype)
    names = ["start", "end"] if column == "both" else [column]
    frame = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in names)
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        frame.select(pi.nesting_depth("start", "end"))


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
        frame.select(pi.nesting_depth("start", "end"))


@pytest.mark.parametrize(
    "dtype", [pl.Float64, pl.Boolean, pl.String, pl.Duration("us"), pl.Time, pl.Int128]
)
def test_unsupported_dtypes_even_when_empty(dtype):
    frame = pl.DataFrame(schema={"start": dtype, "end": dtype})
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype"):
        frame.select(pi.nesting_depth("start", "end"))


def test_invalid_interval_reports_first_original_index():
    frame = pl.DataFrame({"start": [0, 3, 4], "end": [0, 2, 1]})
    with pytest.raises(pl.exceptions.ComputeError, match="index 1"):
        frame.select(pi.nesting_depth("start", "end"))


@pytest.mark.parametrize("start", [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)])
def test_lengths_must_match_without_broadcasting(start):
    frame = pl.DataFrame({"start": [0, 1], "end": [3, 4]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.nesting_depth(start, "end"))


def _naive_depths(intervals):
    @cache
    def depth(j):
        start, end = intervals[j]
        return max(
            (
                1 + depth(i)
                for i, (outer_start, outer_end) in enumerate(intervals)
                if outer_start <= start
                and end <= outer_end
                and (outer_start < start or end < outer_end)
            ),
            default=0,
        )

    return [depth(j) for j in range(len(intervals))]


def test_native_plugin_against_independent_recursive_oracle():
    rng = Random(0xDE971)
    for _ in range(160):
        intervals = [tuple(sorted((rng.randint(-8, 8), rng.randint(-8, 8)))) for _ in range(25)]
        frame = pl.DataFrame(intervals, schema=["start", "end"], orient="row")
        result = frame.select(pi.nesting_depth("start", "end")).to_series().to_list()
        assert result == _naive_depths(intervals)


def test_containment_counts_are_not_depths_and_duplicates_do_not_chain():
    frame = pl.DataFrame({"start": [0, 1, 2, 4, 4], "end": [8, 9, 10, 5, 5]})
    result = frame.select(
        pi.containment_count("start", "end").alias("contains"),
        pi.nesting_depth("start", "end").alias("depth"),
    )
    assert result["contains"].to_list() == [2, 2, 2, 1, 1]
    assert result["depth"].to_list() == [0, 0, 0, 1, 1]
    rows = frame.rows()
    for j, (start, end) in enumerate(rows):
        strict_containers = sum(
            outer_start <= start and end <= outer_end and (outer_start < start or end < outer_end)
            for outer_start, outer_end in rows
        )
        assert result["depth"][j] <= strict_containers
    assert strict_containers == 3
