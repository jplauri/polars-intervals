"""Exact covering through the installed native plugin, including strict scalar config."""

from datetime import UTC, date, datetime, timedelta, timezone
from decimal import Decimal
from itertools import product
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
        for unit, zone in product(("ms", "us", "ns"), (None, "UTC", "Europe/Helsinki"))
    ),
]


def expression(weighted, left=0, right=10, start="start", end="end", cost="cost"):
    kwargs = {"cost": cost} if weighted else {}
    function = pi.minimum_cost_cover if weighted else pi.minimum_cover
    return function(start, end, target_start=left, target_end=right, **kwargs).alias("selected")


def covers(rows, indices, left, right):
    frontier = left
    for s, e, _ in sorted(rows[i] for i in indices):
        if s > frontier:
            break
        frontier = max(frontier, e)
    return frontier >= right


def optimum(rows, weighted, left, right):
    options = []
    for bits in range(1 << len(rows)):
        indices = [i for i in range(len(rows)) if bits & (1 << i)]
        if covers(rows, indices, left, right):
            options.append(
                (sum(rows[i][2] for i in indices) if weighted else len(indices), len(indices))
            )
    return min(options, default=None)


@pytest.mark.parametrize("weighted", [False, True])
@pytest.mark.parametrize(
    "rows,left,right",
    [
        ([], 5, 5),
        ([], 0, 10),
        ([(0, 10, 1)], 0, 10),
        ([(-100, 100, 1)], 0, 10),
        ([(0, 3, 2), (3, 7, 2), (7, 10, 2)], 0, 10),
        ([(0, 4, 1), (5, 10, 1)], 0, 10),
        ([(0, 4, 1), (0, 6, 1), (4, 7, 1), (6, 10, 1), (7, 10, 1)], 0, 10),
        ([(0, 10, 100), (0, 5, 10), (5, 10, 10)], 0, 10),
        ([(0, 4, 1), (0, 6, 5), (4, 10, 100), (6, 10, 5)], 0, 10),
        ([(0, 10, 4), (0, 10, 1), (0, 10, 1)], 0, 10),
        ([(0, 10, 0), (0, 5, 0), (5, 10, 0), (3, 3, 0)], 0, 10),
        ([(7, 12, 3), (-5, 3, 4), (3, 7, 0), (10, 10, 0)], 0, 10),
        ([(0, 0, 1), (5, 5, 0), (10, 10, 3)], 0, 10),
        ([(0, 0, 1), (5, 5, 0), (10, 10, 3)], 5, 5),
        ([(-(2**63), 0, 1), (0, 2**63 - 1, 1)], -(2**63), 2**63 - 1),
    ],
)
def test_oracle_eager_lazy_filter(rows, left, right, weighted):
    df = pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "cost": pl.Int64}, orient="row"
    )
    expected = optimum(rows, weighted, left, right)
    expr = expression(weighted, left, right)
    if expected is None:
        with pytest.raises(pl.exceptions.ComputeError, match="target interval cannot be covered"):
            df.select(expr)
        return
    mask = df.select(expr).to_series()
    indices = [i for i, selected in enumerate(mask) if selected]
    assert covers(rows, indices, left, right)
    assert (
        sum(rows[i][2] for i in indices) if weighted else len(indices),
        len(indices),
    ) == expected
    assert all(rows[i][0] < rows[i][1] for i in indices)
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(df)
    assert mask.equals(df.select(expr).to_series())
    assert mask.equals(df.lazy().with_columns(expr).collect()["selected"])
    assert df.filter(expr).equals(df.filter(mask))
    assert df.lazy().filter(expr).collect().equals(df.filter(mask))


@pytest.mark.parametrize("weighted", [False, True])
@pytest.mark.parametrize("dtype", ENDPOINTS, ids=str)
def test_types_chunks_streaming_groups(dtype, weighted):
    df = pl.DataFrame({"start": [5, 0, 0, 3], "end": [10, 10, 5, 3], "cost": [10, 100, 10, 0]})
    df = df.with_columns(pl.col("start", "end").cast(dtype))
    df = pl.concat([df.head(2), df.tail(2)], rechunk=False)
    assert df["start"].n_chunks() == 2
    target = lambda v: pl.Series([v]).cast(dtype)
    expr = expression(weighted, target(0), target(10))
    expected = [True, False, True, False] if weighted else [False, True, False, False]
    assert df.select(expr).to_series().to_list() == expected
    assert df.lazy().collect_schema() == df.schema
    assert (
        df.lazy().with_columns(expr).collect(engine="streaming")["selected"].to_list() == expected
    )
    assert df.slice(0, 0).select(expression(weighted, target(5), target(5))).height == 0
    assert (
        df.select(expression(weighted, target(5), target(5))).to_series().to_list() == [False] * 4
    )
    grouped = pl.concat([df.with_columns(group=pl.lit("a")), df.with_columns(group=pl.lit("b"))])
    assert grouped.with_columns(expr.over("group"))["selected"].to_list() == expected * 2
    assert grouped.group_by("group").agg(expr)["selected"].to_list() == [expected, expected]
    with pytest.raises(pl.exceptions.ComputeError, match="cannot be covered"):
        grouped.filter(pl.col("end").to_physical() < 10).select(expr.over("group"))


@pytest.mark.parametrize("cost_type", INTEGERS, ids=str)
def test_cost_dtypes_and_expression_arguments(cost_type):
    df = pl.DataFrame({"s": [0, 0, 5], "length": [10, 5, 5], "c": [100, 10, 10]})
    df = df.with_columns(pl.col("c").cast(cost_type))
    result = df.select(
        expression(True, 0, 10, pl.col("s"), pl.col("s") + pl.col("length"), pl.col("c"))
    )
    assert result.to_series().to_list() == [False, True, True]


@pytest.mark.parametrize("weighted", [False, True])
@pytest.mark.parametrize("zone", [None, UTC])
def test_real_temporal_values(weighted, zone):
    start = datetime(2026, 1, 1, 9, tzinfo=zone)
    middle = datetime(2026, 1, 1, 10, tzinfo=zone)
    end = datetime(2026, 1, 1, 11, tzinfo=zone)
    for s, m, e in [(start, middle, end), (date(2026, 1, 1), date(2026, 1, 2), date(2026, 1, 3))]:
        df = pl.DataFrame({"start": [m, s], "end": [e, m], "cost": [1, 1]})
        assert df.select(expression(weighted, s, e)).to_series().to_list() == [True, True]


@pytest.mark.parametrize("weighted", [False, True])
@pytest.mark.parametrize(
    "dtype,target",
    [
        (pl.Date, datetime(2026, 1, 1, tzinfo=UTC).replace(tzinfo=None)),
        (pl.Datetime("us"), date(2026, 1, 1)),
        (pl.Datetime("ms"), datetime(2026, 1, 1, tzinfo=UTC).replace(tzinfo=None)),
        (pl.Datetime("ns"), pl.Series([0]).cast(pl.Datetime("us"))),
        (pl.Datetime("us", "UTC"), datetime(2026, 1, 1, tzinfo=UTC).replace(tzinfo=None)),
        (pl.Datetime("us", "Europe/Helsinki"), datetime(2026, 1, 1, tzinfo=UTC)),
        (pl.Date, 0),
        (pl.Int64, date(2026, 1, 1)),
        (pl.Int64, pl.Series([0], dtype=pl.Int32)),
        (pl.UInt8, -1),
        (pl.Int8, 128),
        (pl.UInt64, 2**64),
        (pl.Int64, 2**200),
    ],
)
def test_incompatible_targets(dtype, target, weighted):
    df = pl.DataFrame({"start": [0], "end": [10], "cost": [1]}).with_columns(
        pl.col("start", "end").cast(dtype)
    )
    with pytest.raises(pl.exceptions.PolarsError, match="target"):
        df.select(expression(weighted, target, target))


@pytest.mark.parametrize("weighted", [False, True])
@pytest.mark.parametrize(
    "target", [True, 1.0, "0", None, Decimal(1), pl.lit(0), pl.Series([1.0]), pl.Series([True])]
)
def test_unsupported_scalar_types(target, weighted):
    with pytest.raises(TypeError, match="target"):
        expression(weighted, target, target)


@pytest.mark.parametrize(
    "target", [pl.Series([], dtype=pl.Int64), pl.Series([1, 2]), pl.Series([None])]
)
def test_non_scalar_series(target):
    with pytest.raises(ValueError, match="exactly one non-null"):
        expression(False, target, target)


def test_datetime_timezone_is_not_silently_normalized():
    target = datetime(2026, 1, 1, tzinfo=timezone(timedelta(hours=2)))
    with pytest.raises(TypeError, match="timezone"):
        expression(False, target, target)


@pytest.mark.parametrize("weighted", [False, True])
def test_invalid_inputs(weighted):
    df = pl.DataFrame({"start": [0, 5], "end": [5, 10], "cost": [1, 1]})
    for frame, left, right, message in [
        (df, 10, 0, "target start"),
        (df.with_columns(start=pl.lit(11, dtype=pl.Int64)), 0, 0, "index 0"),
        (df.with_columns(pl.col("start").cast(pl.Int32)), 0, 10, "matching"),
        (df.with_columns(start=pl.lit(None, dtype=pl.Int64)), 0, 10, "null endpoints"),
        (df.with_columns(end=pl.lit(None, dtype=pl.Int64)), 0, 10, "null endpoints"),
        (df.with_columns(pl.col("start", "end").cast(pl.Float64)), 0, 10, "dtype"),
    ]:
        with pytest.raises(pl.exceptions.PolarsError, match=message):
            frame.select(expression(weighted, left, right))
    with pytest.raises(pl.exceptions.PolarsError, match="equal.*lengths"):
        df.select(expression(weighted, start=pl.col("start").head(1)))


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float32,
        pl.Float64,
        pl.Boolean,
        pl.String,
        pl.Date,
        pl.Datetime,
        pl.Duration,
        pl.Decimal(20, 2),
        pl.Int128,
    ],
)
def test_unsupported_cost_dtype(dtype):
    df = pl.DataFrame({"start": [0], "end": [10], "cost": [1]}).with_columns(
        pl.col("cost").cast(dtype)
    )
    with pytest.raises(pl.exceptions.PolarsError, match="integer cost dtype"):
        df.select(expression(True))


@pytest.mark.parametrize("dtype", [pl.Int8, pl.Int16, pl.Int32, pl.Int64])
def test_negative_null_and_cost_lengths(dtype):
    df = pl.DataFrame({"start": [0, 20], "end": [10, 30], "cost": [1, -1]}).with_columns(
        pl.col("cost").cast(dtype)
    )
    for right in [0, 10]:
        with pytest.raises(pl.exceptions.ComputeError, match="cost at index 1 is negative"):
            df.select(expression(True, right=right))
    with pytest.raises(pl.exceptions.ComputeError, match="null costs"):
        df.with_columns(cost=pl.lit(None, dtype=dtype)).select(expression(True))
    with pytest.raises(pl.exceptions.PolarsError, match="equal interval and cost lengths"):
        df.select(expression(True, cost=pl.col("cost").head(1)))


def test_large_integer_precision():
    df = pl.DataFrame(
        {"start": [0, 0, 5], "end": [5, 5, 2**64 - 1], "cost": [2**64 - 1, 2**64 - 2, 2**64 - 1]},
        schema=pl.Schema({"start": pl.UInt64, "end": pl.UInt64, "cost": pl.UInt64}),
    )
    assert df.select(expression(True, 0, 2**64 - 1)).to_series().to_list() == [False, True, True]


def test_random_plugin_instances_against_subsets():
    rng = Random(20260926)
    for _ in range(80):
        rows = []
        for _ in range(rng.randrange(9)):
            s, e = sorted([rng.randrange(-8, 13), rng.randrange(-8, 13)])
            rows.append((s, e, rng.randrange(21)))
        left, right = sorted([rng.randrange(-8, 13), rng.randrange(-8, 13)])
        for weighted in (False, True):
            test_oracle_eager_lazy_filter(rows, left, right, weighted)
