"""Check coloring properties without promising a particular lane numbering."""

from datetime import UTC, date, datetime
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

ENDPOINT_DTYPES = [
    pl.Int8,
    pl.Int16,
    pl.Int32,
    pl.Int64,
    pl.UInt8,
    pl.UInt16,
    pl.UInt32,
    pl.UInt64,
    pl.Date,
    *(
        pl.Datetime(unit, zone)
        for unit in ("ms", "us", "ns")
        for zone in (None, "UTC", "Europe/Helsinki")
    ),
]


def assert_coloring(frame, lanes):
    starts = frame["start"].to_physical().to_list()
    ends = frame["end"].to_physical().to_list()
    assert lanes.dtype == pl.UInt32
    assert lanes.null_count() == 0
    assert len(lanes) == len(starts)
    labels = lanes.to_list()
    events = sorted(
        event
        for start, end in zip(starts, ends, strict=True)
        if start < end
        for event in [(start, 1), (end, -1)]
    )
    active = peak = 0
    for _, delta in events:
        active += delta
        peak = max(peak, active)
    expected = max(bool(starts), peak)
    assert set(labels) == set(range(expected))
    for i in range(len(starts)):
        for j in range(i):
            if (
                starts[i] < ends[i]
                and starts[j] < ends[j]
                and starts[i] < ends[j]
                and starts[j] < ends[i]
            ):
                assert labels[i] != labels[j]


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
    ],
)
def test_semantics(starts, ends, lazy):
    frame = pl.DataFrame(
        {"start": starts, "end": ends}, schema={"start": pl.Int64, "end": pl.Int64}
    )
    expression = pi.assign_lanes("start", "end").alias("lane")
    assert isinstance(expression, pl.Expr)
    result = frame.lazy().with_columns(expression).collect() if lazy else frame.select(expression)
    assert_coloring(frame, result["lane"])
    assert result["lane"].to_list() == frame.select(expression)["lane"].to_list()


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_supported_dtypes_chunks_slices_and_shuffled_rows(dtype, engine):
    intervals = [(i // 4, i // 4 + i % 5) for i in range(80)]
    Random(42).shuffle(intervals)
    starts, ends = zip(*intervals, strict=True)
    frame = pl.DataFrame({"start": starts, "end": ends}).with_columns(pl.all().cast(dtype))
    frame = pl.concat([frame.head(23), frame.slice(23, 31), frame.slice(54)], rechunk=False)
    assert frame["start"].n_chunks() == 3
    expression = pi.assign_lanes(pl.col("start"), "end").alias("lane")
    for subset in [frame, frame.slice(3, 60), frame.head(0)]:
        query = subset.lazy().with_columns(expression)
        assert query.collect_schema()["lane"] == pl.UInt32
        lanes = query.collect(engine=engine)["lane"]
        assert_coloring(subset, lanes)
        assert lanes.to_list() == subset.rechunk().select(expression)["lane"].to_list()


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_window_and_grouped_aggregation(dtype):
    frame = pl.DataFrame({"start": [0, 20, 1, 22, 2, 24, 3], "end": [10, 23, 9, 25, 8, 26, 7]})
    frame = frame.with_columns(
        pl.all().cast(dtype), pl.Series("group", ["a", "b", "a", "b", "a", "b", "a"])
    )
    expression = pi.assign_lanes("start", "end").alias("lane")
    result = frame.lazy().with_columns(expression.over("group")).collect()
    grouped = frame.lazy().group_by("group", maintain_order=True).agg(expression).collect()
    assert grouped.schema["lane"] == pl.List(pl.UInt32)
    for group, labels in grouped.iter_rows():
        subset = result.filter(pl.col("group") == group)
        assert_coloring(subset, subset["lane"])
        assert labels == subset["lane"].to_list()


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_validation(dtype):
    frame = pl.DataFrame({"start": [2, 0, 5, 6], "end": [2, 1, 4, 8]}).with_columns(
        pl.all().cast(dtype)
    )
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        frame.lazy().select(pi.assign_lanes("start", "end")).collect()
    for columns in [["start"], ["end"], ["start", "end"]]:
        null_frame = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in columns)
        with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
            null_frame.select(pi.assign_lanes("start", "end"))


@pytest.mark.parametrize(
    "start_dtype,end_dtype",
    [
        (pl.Int32, pl.Int64),
        (pl.Int32, pl.UInt32),
        (pl.Date, pl.Int32),
        (pl.Date, pl.Datetime("ms")),
        (pl.Datetime("ms"), pl.Int64),
        (pl.Datetime("ms"), pl.Datetime("us")),
        (pl.Datetime("ns", "UTC"), pl.Datetime("ns", "Europe/Helsinki")),
        (pl.Datetime("us"), pl.Datetime("us", "UTC")),
    ],
)
def test_no_logical_dtype_coercion(start_dtype, end_dtype):
    for values in [[0], []]:
        frame = pl.DataFrame(
            {
                "start": pl.Series(values, dtype=start_dtype),
                "end": pl.Series(values, dtype=end_dtype),
            }
        )
        with pytest.raises(
            pl.exceptions.PolarsError, match="matching integer, Date, or Datetime dtypes"
        ):
            frame.select(pi.assign_lanes("start", "end"))


@pytest.mark.parametrize(
    "dtype",
    [pl.Float32, pl.Float64, pl.Boolean, pl.String, pl.Null, pl.Time, pl.Duration("us")],
)
def test_unsupported_dtypes_including_empty(dtype):
    for values in [[None], []]:
        frame = pl.DataFrame({name: pl.Series(values, dtype=dtype) for name in ["start", "end"]})
        with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
            frame.select(pi.assign_lanes("start", "end"))


@pytest.mark.parametrize("algorithm", [pi.assign_lanes, pi.overlap_count])
def test_disabled_polars_dtype_feature_raises_instead_of_aborting(algorithm):
    # Int128 field import can fail before the adapter's validation is reached.
    # The plugin schema callback must catch that failure at its FFI boundary.
    for values in [[0], [None], []]:
        frame = pl.DataFrame(
            {name: pl.Series(values, dtype=pl.Int128) for name in ["start", "end"]}
        )
        with pytest.raises(pl.exceptions.PolarsError):
            frame.select(algorithm("start", "end"))


@pytest.mark.parametrize("start", [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)])
def test_no_broadcasting(start):
    frame = pl.DataFrame({"start": [1, 2], "end": [3, 4]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.assign_lanes(start, "end"))


def test_real_dates_and_datetimes_across_daylight_saving_change():
    for dtype, starts, ends in [
        (pl.Date, [date(2026, 1, d) for d in [1, 2, 3]], [date(2026, 1, d) for d in [3, 4, 5]]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in [(0, 30), (1, 0), (0, 45)]],
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in [(1, 0), (1, 30), (1, 15)]],
        ),
    ]:
        frame = pl.DataFrame({"start": starts, "end": ends}, schema={"start": dtype, "end": dtype})
        assert_coloring(frame, frame.select(pi.assign_lanes("start", "end")).to_series())


@pytest.mark.parametrize("dtype", [pl.UInt64, pl.Int64, pl.Datetime("ns")])
def test_extreme_values_and_single_ticks(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    minimum = 0 if dtype == pl.UInt64 else -(2**63)
    frame = pl.DataFrame(
        {"start": [minimum, maximum - 2, maximum - 1, maximum], "end": [maximum] * 4},
        schema={"start": dtype, "end": dtype},
    )
    assert_coloring(frame, frame.select(pi.assign_lanes("start", "end")).to_series())
