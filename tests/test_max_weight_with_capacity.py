"""Capacity-constrained optimization through the compiled Polars expression."""

from datetime import UTC, date, datetime
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

INTEGERS = [pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64]
ENDPOINTS = [
    *INTEGERS,
    pl.Date,
    *(
        pl.Datetime(unit, zone)
        for unit in ("ms", "us", "ns")
        for zone in (None, "UTC", "Europe/Helsinki")
    ),
]


def expression(k):
    return pi.max_weight_with_capacity("start", "end", weight="weight", capacity=k).alias(
        "selected"
    )


def assert_optimal(frame, mask, k):
    s, e = (frame[name].to_physical().to_list() for name in ("start", "end"))
    w = frame["weight"].to_list()
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(frame)

    def feasible(indices):
        return all(sum(s[i] <= t < e[i] for i in indices) <= k for t in s)

    optimum = 0
    for bits in range(1 << len(frame)):
        indices = [i for i in range(len(frame)) if bits & (1 << i)]
        if feasible(indices):
            optimum = max(optimum, sum(w[i] for i in indices))
    chosen = [i for i, value in enumerate(mask) if value]
    assert feasible(chosen)
    assert sum(w[i] for i in chosen) == optimum
    assert all(w[i] > 0 for i in chosen)
    assert all(mask[i] for i in range(len(frame)) if s[i] == e[i] and w[i] > 0)


@pytest.mark.parametrize("k", [0, 1, 2, 4, 64, 2**64 - 1])
@pytest.mark.parametrize(
    "rows",
    [
        [],
        [(0, 4, -1), (2, 2, -2)],
        [(0, 1, 3), (1, 2, 4), (2, 2, 0)],
        [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10)],
        [(0, 2, 3), (1, 3, 2), (2, 4, 2), (3, 5, 3)],
        [(0, 5, w) for w in (9, 7, 4, 2, 0, -1)] + [(2, 2, 3)],
        [(0, 5, 2)] * 5 + [(2, 2, 8), (2, 2, 0), (2, 2, -1)],
    ],
)
def test_eager_lazy_filter_empty_and_deterministic(rows, k):
    frame = pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "weight": pl.Int64}, orient="row"
    )
    mask = frame.select(expression(k)).to_series()
    assert_optimal(frame, mask, k)
    assert mask.equals(frame.select(expression(k)).to_series())
    assert mask.equals(frame.lazy().with_columns(expression(k)).collect()["selected"])
    assert frame.filter(expression(k)).equals(frame.filter(mask))
    assert frame.lazy().filter(expression(k)).collect().equals(frame.filter(mask))
    if k == 1:
        assert mask.equals(
            frame.select(
                pi.max_weight_non_overlapping("start", "end", weight="weight").alias("selected")
            ).to_series()
        )


@pytest.mark.parametrize("endpoint", ENDPOINTS, ids=str)
@pytest.mark.parametrize("weight", INTEGERS, ids=str)
@pytest.mark.parametrize("k", [0, 1, 2])
def test_dtypes_chunks_shuffled_slices_streaming(endpoint, weight, k):
    rows = [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10), (1, 9, 25), (2, 2, 8)]
    Random(42).shuffle(rows)
    frame = pl.DataFrame(rows, schema=["start", "end", "weight"], orient="row").with_columns(
        pl.col("start", "end").cast(endpoint), pl.col("weight").cast(weight)
    )
    frame = pl.concat([frame.head(2), frame.slice(2, 2), frame.slice(4)], rechunk=False)
    assert all(s.n_chunks() == 3 for s in frame)
    for subset in (frame, frame.slice(1, 4), frame.head(0)):
        query = subset.lazy().with_columns(expression(k))
        assert query.collect_schema()["selected"] == pl.Boolean
        mask = query.collect(engine="streaming")["selected"]
        assert_optimal(subset, mask, k)
        assert mask.equals(subset.rechunk().select(expression(k)).to_series())


def test_expression_arguments():
    frame = pl.DataFrame({"s": [0, 0, 0], "length": [3, 3, 3], "w": [3, 7, 5]})
    mask = frame.select(
        pi.max_weight_with_capacity(
            pl.col("s") + 100,
            pl.col("s") + pl.col("length") + 100,
            weight=pl.col("w") * 2,
            capacity=2,
        )
    ).to_series()
    assert mask.to_list() == [False, True, True]


@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("ns", "UTC")], ids=str)
@pytest.mark.parametrize("k", [0, 1, 2, 8])
def test_window_and_grouped_execution(dtype, k):
    frame = pl.DataFrame(
        {
            "start": [0] * 8,
            "end": [5, 5, 5, 5, 5, 5, 0, 0],
            "weight": [10, 1, 5, 9, 1, 4, 3, 2],
            "group": ["a", "b"] * 4,
        }
    ).with_columns(pl.col("start", "end").cast(dtype))
    window = frame.lazy().with_columns(expression(k).over("group")).collect()
    grouped = frame.lazy().group_by("group", maintain_order=True).agg(expression(k)).collect()
    assert grouped.schema["selected"] == pl.List(pl.Boolean)
    for group, labels in grouped.iter_rows():
        subset = window.filter(pl.col("group") == group)
        assert_optimal(subset, subset["selected"], k)
        assert labels == subset["selected"].to_list()


@pytest.mark.parametrize("k", [0, 1, 2])
@pytest.mark.parametrize("dtype", ENDPOINTS, ids=str)
def test_invalid_and_null_endpoints(k, dtype):
    frame = pl.DataFrame({"start": [2, 0, 5], "end": [2, 1, 4], "weight": [1, 0, -1]}).with_columns(
        pl.col("start", "end").cast(dtype)
    )
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        frame.select(expression(k))
    for name in ("start", "end"):
        with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
            frame.with_columns(pl.lit(None, dtype=dtype).alias(name)).select(expression(k))


@pytest.mark.parametrize("k", [0, 1, 2])
@pytest.mark.parametrize("dtype", INTEGERS, ids=str)
def test_null_weights(k, dtype):
    frame = pl.DataFrame({"start": [0], "end": [1], "weight": pl.Series([None], dtype=dtype)})
    with pytest.raises(pl.exceptions.PolarsError, match="null weights"):
        frame.select(expression(k))


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float32,
        pl.Float64,
        pl.Int128,
        pl.Decimal(20, 2),
        pl.Boolean,
        pl.String,
        pl.Null,
        pl.Date,
        pl.Datetime("us"),
        pl.Time,
        pl.Duration("us"),
    ],
    ids=str,
)
def test_unsupported_weights(dtype):
    for values in ([], [None]):
        frame = pl.DataFrame(
            {
                "start": pl.Series([0] * len(values), dtype=pl.Int64),
                "end": pl.Series([1] * len(values), dtype=pl.Int64),
                "weight": pl.Series(values, dtype=dtype),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
            frame.select(expression(2))


@pytest.mark.parametrize(
    "start,end",
    [
        (pl.Int32, pl.Int64),
        (pl.Date, pl.Int32),
        (pl.Datetime("ms"), pl.Datetime("us")),
        (pl.Datetime("ns", "UTC"), pl.Datetime("ns", "Europe/Helsinki")),
    ],
)
def test_mismatched_endpoints(start, end):
    frame = pl.DataFrame(
        {"start": pl.Series([0], dtype=start), "end": pl.Series([1], dtype=end), "weight": [1]}
    )
    with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
        frame.select(expression(2))


@pytest.mark.parametrize("dtype", [pl.Float64, pl.Boolean, pl.String, pl.Time, pl.Duration("us")])
def test_unsupported_endpoints(dtype):
    frame = pl.DataFrame(
        {name: pl.Series([], dtype=dtype) for name in ("start", "end")}
    ).with_columns(pl.Series("weight", [], dtype=pl.Int64))
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
        frame.select(expression(2))


@pytest.mark.parametrize("argument", ["start", "end", "weight"])
@pytest.mark.parametrize("scalar", [False, True])
def test_no_broadcasting(argument, scalar):
    args = {name: pl.col(name) for name in ("start", "end", "weight")}
    args[argument] = pl.lit(1, dtype=pl.Int64) if scalar else args[argument].head(1)
    frame = pl.DataFrame({"start": [0, 1], "end": [1, 2], "weight": [1, 2]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.max_weight_with_capacity(**args, capacity=2))


@pytest.mark.parametrize(
    "capacity,error",
    [
        (-1, ValueError),
        (2**64, ValueError),
        (1.5, TypeError),
        (True, TypeError),
        (None, TypeError),
        ("2", TypeError),
    ],
)
def test_invalid_capacity(capacity, error):
    with pytest.raises(error, match="capacity"):
        expression(capacity)


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_large_weights(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    frame = pl.DataFrame(
        {
            "start": [0, 0, 0, 2],
            "end": [2, 2, 2, 3],
            "weight": pl.Series([maximum - 2, maximum, maximum - 1, maximum], dtype=dtype),
        }
    )
    mask = frame.select(expression(2)).to_series()
    assert mask.to_list() == [False, True, True, True]
    assert_optimal(frame, mask, 2)


def test_real_temporal_endpoints():
    for dtype, values in [
        (pl.Date, [date(2026, 1, d) for d in (1, 2, 3)]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((0, 30), (1, 0), (1, 30))],
        ),
    ]:
        frame = pl.DataFrame(
            {
                "start": [values[0]] * 4,
                "end": [values[2]] * 3 + [values[0]],
                "weight": [9, 7, 5, 3],
            },
            schema={"start": dtype, "end": dtype, "weight": pl.Int64},
        )
        assert_optimal(frame, frame.select(expression(2)).to_series(), 2)
