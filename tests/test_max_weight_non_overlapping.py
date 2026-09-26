"""Exact objectives and feasibility, without prescribing which optimum wins ties."""

from datetime import UTC, date, datetime
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

INTEGER_DTYPES = [pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64]
ENDPOINT_DTYPES = [
    *INTEGER_DTYPES,
    pl.Date,
    *(
        pl.Datetime(unit, zone)
        for unit in ("ms", "us", "ns")
        for zone in (None, "UTC", "Europe/Helsinki")
    ),
]


def expression():
    return pi.max_weight_non_overlapping("start", "end", weight="weight").alias("selected")


def assert_optimal(frame, mask):
    starts = frame["start"].to_physical().to_list()
    ends = frame["end"].to_physical().to_list()
    weights = frame["weight"].to_list()
    assert mask.dtype == pl.Boolean
    assert mask.null_count() == 0
    assert len(mask) == len(frame)

    def feasible(indices):
        return all(
            starts[i] == ends[i]
            or starts[j] == ends[j]
            or ends[i] <= starts[j]
            or ends[j] <= starts[i]
            for i in indices
            for j in indices
            if i < j
        )

    optimum = 0
    for bits in range(1 << len(frame)):
        indices = [i for i in range(len(frame)) if bits & (1 << i)]
        if feasible(indices):
            optimum = max(optimum, sum(weights[i] for i in indices))
    chosen = [i for i, selected in enumerate(mask) if selected]
    assert feasible(chosen)
    assert sum(weights[i] for i in chosen) == optimum
    assert all(weights[i] > 0 for i in chosen)
    assert all(mask[i] for i in range(len(frame)) if starts[i] == ends[i] and weights[i] > 0)


@pytest.mark.parametrize(
    "rows",
    [
        [],
        [(1, 5, 10)],
        [(1, 5, -10)],
        [(1, 5, 0)],
        [(0, 1, -1), (2, 3, -2)],
        [(0, 1, 10), (1, 2, 20)],
        [(0, 5, 10), (2, 4, 20)],
        [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10)],
        [(0, 1, 1), (0, 4, 20)],
        [(1, 5, 10)] * 4,
        [(0, 5, 10), (2, 2, 3), (2, 2, -4), (2, 2, 0), (2, 2, 5), (5, 8, -9)],
    ],
)
def test_eager_lazy_filter_and_determinism(rows):
    frame = pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "weight": pl.Int64}, orient="row"
    )
    mask = frame.select(expression()).to_series()
    assert_optimal(frame, mask)
    assert mask.equals(frame.lazy().with_columns(expression()).collect()["selected"])
    assert frame.filter(expression()).equals(frame.filter(mask))
    assert frame.lazy().filter(expression()).collect().equals(frame.filter(mask))
    assert mask.equals(frame.select(expression()).to_series())


@pytest.mark.parametrize("endpoint_dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("weight_dtype", INTEGER_DTYPES, ids=str)
def test_dtypes_chunks_slices_and_shuffled_order(endpoint_dtype, weight_dtype):
    rows = [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10), (2, 2, 3), (2, 2, 4), (1, 5, 0)]
    Random(42).shuffle(rows)
    frame = pl.DataFrame(rows, schema=["start", "end", "weight"], orient="row").with_columns(
        pl.col("start", "end").cast(endpoint_dtype), pl.col("weight").cast(weight_dtype)
    )
    frame = pl.concat([frame.head(2), frame.slice(2, 3), frame.slice(5)], rechunk=False)
    assert all(series.n_chunks() == 3 for series in frame)
    for subset in (frame, frame.slice(1, 5), frame.head(0)):
        query = subset.lazy().with_columns(expression())
        assert query.collect_schema()["selected"] == pl.Boolean
        mask = query.collect(engine="streaming")["selected"]
        assert_optimal(subset, mask)
        assert mask.equals(subset.rechunk().select(expression()).to_series())


def test_expression_arguments():
    frame = pl.DataFrame({"s": [0, 0, 4, 7], "length": [10, 4, 3, 3], "revenue": [15, 10, 10, 10]})
    mask = frame.select(
        pi.max_weight_non_overlapping(
            pl.col("s") + 100,
            pl.col("s") + pl.col("length") + 100,
            weight=pl.col("revenue") * 2,
        )
    ).to_series()
    assert mask.to_list() == [False, True, True, True]


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_window_and_grouped_aggregation(dtype):
    frame = pl.DataFrame(
        {
            "start": [0, 0, 0, 0, 4, 4],
            "end": [10, 10, 4, 4, 10, 10],
            "weight": [15, 100, 10, 10, 10, 10],
            "group": ["a", "b", "a", "b", "a", "b"],
        }
    ).with_columns(pl.col("start", "end").cast(dtype))
    window = frame.lazy().with_columns(expression().over("group")).collect()
    grouped = frame.lazy().group_by("group", maintain_order=True).agg(expression()).collect()
    assert grouped.schema["selected"] == pl.List(pl.Boolean)
    for group, labels in grouped.iter_rows():
        subset = window.filter(pl.col("group") == group)
        assert_optimal(subset, subset["selected"])
        assert labels == subset["selected"].to_list()


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_invalid_and_null_endpoints(dtype):
    frame = pl.DataFrame({"start": [2, 0, 5], "end": [2, 1, 4], "weight": [1, 0, -1]}).with_columns(
        pl.col("start", "end").cast(dtype)
    )
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        frame.select(expression())
    for names in (["start"], ["end"], ["start", "end"]):
        null_frame = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in names)
        with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
            null_frame.select(expression())


@pytest.mark.parametrize("dtype", INTEGER_DTYPES, ids=str)
def test_null_weights(dtype):
    frame = pl.DataFrame({"start": [0], "end": [1], "weight": pl.Series([None], dtype=dtype)})
    with pytest.raises(pl.exceptions.ComputeError, match="null weights"):
        frame.select(expression())


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
def test_mismatched_endpoints(start_dtype, end_dtype):
    for values in ([0], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series(values, dtype=start_dtype),
                "end": pl.Series(values, dtype=end_dtype),
                "weight": pl.Series(values, dtype=pl.Int64),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
            frame.select(expression())


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float32,
        pl.Float64,
        pl.Boolean,
        pl.String,
        pl.Null,
        pl.Date,
        pl.Datetime("us"),
        pl.Time,
        pl.Duration("us"),
    ],
)
def test_unsupported_weights(dtype):
    frame = pl.DataFrame(
        {
            "start": pl.Series([], dtype=pl.Int64),
            "end": pl.Series([], dtype=pl.Int64),
            "weight": pl.Series([], dtype=dtype),
        }
    )
    with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
        frame.select(expression())


@pytest.mark.parametrize("dtype,value", [(pl.Float64, 1.5), (pl.Boolean, True), (pl.String, "1")])
def test_nonempty_unsupported_weights(dtype, value):
    frame = pl.DataFrame({"start": [0], "end": [1], "weight": pl.Series([value], dtype=dtype)})
    with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
        frame.select(expression())


@pytest.mark.parametrize("dtype", [pl.Int128, pl.Decimal(20, 2)])
def test_decimal_and_int128_have_explicit_dtype_errors(dtype):
    for values in ([1], [None], []):
        weights = pl.Series(values).cast(dtype)
        frame = pl.DataFrame(
            {
                "start": pl.Series([0] * len(values), dtype=pl.Int64),
                "end": pl.Series([1] * len(values), dtype=pl.Int64),
                "weight": weights,
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
            frame.select(expression())


@pytest.mark.parametrize("dtype", [pl.Float64, pl.Boolean, pl.String, pl.Time, pl.Duration("us")])
def test_unsupported_endpoints(dtype):
    frame = pl.DataFrame(
        {name: pl.Series([], dtype=dtype) for name in ("start", "end")}
    ).with_columns(pl.Series("weight", [], dtype=pl.Int64))
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
        frame.select(expression())


@pytest.mark.parametrize("argument", ["start", "end", "weight"])
@pytest.mark.parametrize("scalar", [False, True])
def test_no_broadcasting(argument, scalar):
    args = {name: pl.col(name) for name in ("start", "end", "weight")}
    args[argument] = pl.lit(1, dtype=pl.Int64) if scalar else args[argument].head(1)
    frame = pl.DataFrame({"start": [0, 1], "end": [1, 2], "weight": [1, 2]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.max_weight_non_overlapping(**args))


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_large_weights(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    # A float roundtrip would erase the one-unit difference in this conflict.
    frame = pl.DataFrame(
        {
            "start": [0, 0, 2],
            "end": [2, 2, 3],
            "weight": pl.Series([maximum - 1, maximum, maximum], dtype=dtype),
        }
    )
    mask = frame.select(expression()).to_series()
    assert mask.to_list() == [False, True, True]
    assert_optimal(frame, mask)


def test_real_temporal_endpoints():
    for dtype, starts, ends in [
        (pl.Date, [date(2026, 1, d) for d in (1, 2, 3)], [date(2026, 1, d) for d in (3, 3, 4)]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((0, 30), (1, 0), (0, 45))],
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((1, 0), (1, 30), (1, 15))],
        ),
    ]:
        frame = pl.DataFrame(
            {"start": starts, "end": ends, "weight": [10, 10, 15]},
            schema={"start": dtype, "end": dtype, "weight": pl.Int64},
        )
        assert_optimal(frame, frame.select(expression()).to_series())
