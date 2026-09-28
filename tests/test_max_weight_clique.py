"""Exercise the compiled plugin against an independent subset oracle."""

from datetime import UTC, date, datetime
from itertools import combinations
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES


def expression(weight="value"):
    return pi.max_weight_clique("start", "end", weight=weight).alias("selected")


def frame_from_rows(rows):
    return pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "value": pl.Int64}, orient="row"
    )


def assert_optimal(frame, mask, *, units=False):
    starts = frame["start"].to_physical().to_list()
    ends = frame["end"].to_physical().to_list()
    weights = [1] * len(frame) if units else frame["value"].to_list()
    assert mask.dtype == pl.Boolean
    assert mask.null_count() == 0
    assert len(mask) == len(frame)

    def feasible(indices):
        return all(
            starts[i] < ends[i]
            and starts[j] < ends[j]
            and starts[i] < ends[j]
            and starts[j] < ends[i]
            for i, j in combinations(indices, 2)
        )

    best = 0
    for bits in range(1 << len(frame)):
        indices = [i for i in range(len(frame)) if bits & (1 << i)]
        if feasible(indices):
            best = max(best, sum(weights[i] for i in indices))
    chosen = [i for i, selected in enumerate(mask) if selected]
    assert feasible(chosen)
    assert all(weights[i] > 0 for i in chosen)
    assert sum(weights[i] for i in chosen) == best


@pytest.mark.parametrize(
    "rows,expected",
    [
        ([], []),
        ([(1, 3, 5)], [True]),
        ([(1, 1, 5)], [True]),
        ([(1, 3, 0)], [False]),
        ([(1, 1, -1)], [False]),
        ([(0, 0, 1), (5, 6, 1), (9, 9, 1)], [False, True, False]),
        ([(1, 1, 7), (1, 1, 7), (0, 0, 2)], [True, False, False]),
        ([(0, 2, 5), (2, 4, 7)], [False, True]),
        ([(0, 2, 5), (2, 4, 7), (1, 3, 4)], [False, True, True]),
        ([(0, 2, 4), (1, 3, 5), (2, 4, 6)], [False, True, True]),
        ([(0, 10, 2), (1, 9, 3), (2, 8, 5)], [True, True, True]),
        ([(1, 5, 2), (1, 5, 3), (1, 5, -7), (1, 5, 0)], [True, True, False, False]),
        ([(0, 5, 10), (1, 4, -100), (2, 3, 9)], [True, False, True]),
        ([(2, 2, 19), (0, 5, 10), (2, 3, 9), (2, 2, 19)], [False, True, True, False]),
        ([(2, 2, 20), (0, 5, 10), (2, 3, 9), (2, 2, 20)], [True, False, False, False]),
        # Equal objective at distinct coordinates uses the earliest, regardless of row order.
        ([(5, 6, 4), (0, 2, 4)], [False, True]),
        # No cardinality tie objective: earliest singleton ties a later pair.
        ([(0, 1, 6), (3, 5, 3), (3, 5, 3)], [True, False, False]),
        ([(20, 22, 3), (0, 3, 5), (21, 23, 3), (1, 2, 4)], [False, True, False, True]),
        ([(0, 2, 2), (0, 2, 3), (2, 4, 4), (2, 4, 5)], [False, False, True, True]),
    ],
)
def test_semantics_contexts_and_determinism(rows, expected):
    frame = frame_from_rows(rows)
    mask = frame.select(expression()).to_series()
    assert mask.to_list() == expected
    assert_optimal(frame, mask)
    assert mask.equals(frame.lazy().select(expression()).collect().to_series())
    assert mask.equals(frame.with_columns(expression())["selected"])
    assert mask.equals(frame.lazy().with_columns(expression()).collect()["selected"])
    assert frame.filter(expression()).equals(frame.filter(mask))
    assert frame.lazy().filter(expression()).collect().equals(frame.filter(mask))
    assert mask.equals(frame.select(expression()).to_series())
    unit = frame.select(expression(None)).to_series()
    assert_optimal(frame, unit, units=True)
    assert unit.sum() == frame.select(pi.assign_lanes("start", "end")).to_series().n_unique()


def test_units_ignore_named_weight_column_and_expressions_are_supported():
    frame = frame_from_rows([(0, 5, 1), (1, 4, 1), (2, 3, 1), (10, 11, 9)]).with_columns(
        pl.Series("weight", [0, 0, 0, 100]), pl.Series("ones", [1, 1, 1, 1])
    )
    expected = [True, True, True, False]
    for expr in (
        pi.max_weight_clique("start", "end"),
        pi.max_weight_clique("start", "end", weight=None),
        pi.max_weight_clique(pl.col("start"), pl.col("end"), weight="ones"),
    ):
        assert frame.select(expr).to_series().to_list() == expected
    assert frame.select(expression()).to_series().to_list() == [False, False, False, True]
    assert frame.select(
        pi.max_weight_clique(
            pl.col("start") + 100,
            pl.col("end") + 100,
            weight=pl.col("value") * 2,
        )
    ).to_series().to_list() == [False, False, False, True]
    assert "max_weight_clique" in pi.__all__


def test_none_is_configuration_while_null_expression_is_invalid():
    frame = frame_from_rows([(1, 1, -1)])
    assert frame.select(expression(None)).to_series().to_list() == [True]
    with pytest.raises(pl.exceptions.PolarsError, match="null weights"):
        frame.select(pi.max_weight_clique("start", "end", weight=pl.lit(None, dtype=pl.Int64)))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_endpoint_types_schema_and_unit_equivalence(dtype):
    frame = frame_from_rows([(4, 6, 1), (0, 5, 1), (1, 4, 1), (2, 3, 1), (1, 1, 1)]).with_columns(
        pl.col("start", "end").cast(dtype)
    )
    for subset in (frame, frame.slice(1, 3), frame.head(0), frame.head(1)):
        query = subset.lazy().with_columns(expression())
        assert query.collect_schema()["selected"] == pl.Boolean
        mask = query.collect()["selected"]
        assert_optimal(subset, mask)
        assert mask.equals(subset.select(expression(None)).to_series())


@pytest.mark.parametrize("dtype", INTEGER_DTYPES, ids=str)
def test_weight_types(dtype):
    frame = frame_from_rows([(0, 4, 2), (1, 3, 3), (2, 2, 4), (10, 11, 0)]).with_columns(
        pl.col("value").cast(dtype)
    )
    assert frame.select(expression()).to_series().to_list() == [True, True, False, False]
    assert_optimal(frame, frame.select(expression()).to_series())


@pytest.mark.parametrize("weight", [None, "value"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_different_column_chunk_boundaries_and_global_reconstruction(weight, engine):
    frame = frame_from_rows([(8, 9, 1), (0, 5, 4), (12, 13, 2), (1, 4, 5), (2, 3, 6), (7, 7, 9)])
    chunked = pl.DataFrame(
        [
            pl.concat([frame[name][:cut], frame[name][cut:]], rechunk=False)
            for name, cut in (("start", 2), ("end", 4), ("value", 3))
        ]
    )
    assert [series.chunk_lengths() for series in chunked] == [[2, 4], [4, 2], [3, 3]]
    for subset in (chunked, chunked.slice(1, 4), chunked.head(0)):
        mask = subset.lazy().select(expression(weight)).collect(engine=engine).to_series()
        assert_optimal(subset, mask, units=weight is None)
        assert mask.equals(subset.rechunk().select(expression(weight)).to_series())
    assert chunked.select(expression(weight)).to_series().to_list() == [
        False,
        True,
        False,
        True,
        True,
        False,
    ]


@pytest.mark.parametrize(
    "dtype", [pl.Int64, pl.Date, pl.Datetime("ns", "Europe/Helsinki")], ids=str
)
@pytest.mark.parametrize("weight", [None, "value"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_interleaved_whole_groups_and_aggregation(dtype, weight, engine):
    frame = frame_from_rows(
        [(0, 3, 4), (5, 5, 7), (0, 4, 0), (1, 2, 5), (5, 5, 8), (1, 3, -2), (8, 9, 1), (6, 6, 8)]
    ).with_columns(
        pl.col("start", "end").cast(dtype),
        pl.Series("group", ["a", "b", "c", "a", "b", "c", "a", "b"]),
    )
    frame = pl.concat([frame[:3], frame[3:6], frame[6:]], rechunk=False)
    window = frame.lazy().with_columns(expression(weight).over("group")).collect(engine=engine)
    eager = frame.with_columns(expression(weight).over("group"))
    assert window.equals(eager)
    query = frame.lazy().group_by("group", maintain_order=True).agg(expression(weight))
    assert query.collect_schema()["selected"] == pl.List(pl.Boolean)
    grouped = query.collect(engine=engine)
    for group, masks in grouped.iter_rows():
        subset = window.filter(pl.col("group") == group)
        independent = subset.select(expression(weight)).to_series()
        assert_optimal(subset, independent, units=weight is None)
        assert independent.to_list() == masks == subset["selected"].to_list()


def test_python_subset_oracle_deterministic_random_instances_and_groups():
    rng = Random(76109)
    for _ in range(50):
        rows = []
        for _ in range(rng.randrange(10)):
            start, end = sorted([rng.randrange(-4, 6), rng.randrange(-4, 6)])
            rows.append((start, end, rng.randrange(-5, 11)))
        frame = frame_from_rows(rows)
        for weight in (None, "value"):
            mask = frame.select(expression(weight)).to_series()
            assert_optimal(frame, mask, units=weight is None)
            assert mask.equals(
                frame.lazy().select(expression(weight)).collect(engine="streaming").to_series()
            )
        grouped = frame.with_columns(pl.Series("group", [i % 3 for i in range(len(rows))]))
        result = grouped.with_columns(expression().over("group"))
        for subset in result.partition_by("group"):
            assert_optimal(subset, subset["selected"])


@pytest.mark.parametrize("weight", [None, "value"])
@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_reversed_rows_are_validated_even_with_nonpositive_or_empty_candidates(dtype, weight):
    for rows in (
        [(0, 0, 0), (1, 1, -2), (5, 4, -1)],
        [(0, 5, 10), (1, 4, 10), (5, 4, 0)],
    ):
        frame = frame_from_rows(rows).with_columns(pl.col("start", "end").cast(dtype))
        with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
            frame.select(expression(weight))


def test_reversed_group_error_uses_original_group_order():
    frame = frame_from_rows([(1, 1, 0), (0, 0, 0), (1, 1, -1), (5, 4, -1)]).with_columns(
        pl.Series("group", ["b", "a", "a", "a"])
    )
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        frame.select(expression().over("group"))


@pytest.mark.parametrize("argument", ["start", "end", "value"])
def test_null_in_each_argument(argument):
    frame = frame_from_rows([(0, 0, -1), (1, 2, 0)]).with_columns(
        pl.Series(argument, [0, None], dtype=pl.Int64)
    )
    message = "null weights" if argument == "value" else "null endpoints"
    with pytest.raises(pl.exceptions.PolarsError, match=message):
        frame.select(expression())
    if argument != "value":
        with pytest.raises(pl.exceptions.PolarsError, match=message):
            frame.select(expression(None))


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
def test_matching_logical_dtypes_at_plan_time(start_dtype, end_dtype):
    for values in ([0], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series(values, dtype=start_dtype),
                "end": pl.Series(values, dtype=end_dtype),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
            frame.lazy().select(expression(None)).collect_schema()
        with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
            frame.select(expression(None))


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float32,
        pl.Float64,
        pl.Boolean,
        pl.String,
        pl.Null,
        pl.Time,
        pl.Duration("us"),
        pl.Int128,
        pl.Decimal(20, 2),
    ],
    ids=str,
)
def test_unsupported_endpoints_are_safe_polars_errors_at_schema_and_runtime(dtype):
    for values in ([None], []):
        frame = pl.DataFrame({name: pl.Series(values, dtype=dtype) for name in ("start", "end")})
        with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
            frame.lazy().select(expression(None)).collect_schema()
        with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
            frame.select(expression(None))


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
        pl.Int128,
        pl.Decimal(20, 2),
    ],
    ids=str,
)
def test_unsupported_weights_at_schema_and_runtime(dtype):
    for values in ([None], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series([0] * len(values), dtype=pl.Int64),
                "end": pl.Series([0] * len(values), dtype=pl.Int64),
                "value": pl.Series(values, dtype=dtype),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
            frame.lazy().select(expression()).collect_schema()
        with pytest.raises(pl.exceptions.PolarsError, match="integer weight dtype"):
            frame.select(expression())


@pytest.mark.parametrize("argument", ["start", "end", "weight"])
@pytest.mark.parametrize("scalar", [False, True])
def test_no_scalar_or_short_expression_broadcasting(argument, scalar):
    args = {"start": pl.col("start"), "end": pl.col("end"), "weight": pl.col("value")}
    args[argument] = pl.lit(1, dtype=pl.Int64) if scalar else args[argument].head(1)
    frame = frame_from_rows([(0, 2, 1), (1, 3, 2)])
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(pi.max_weight_clique(**args))


@pytest.mark.parametrize(
    "dtype",
    [pl.Int64, pl.UInt64, pl.Date, pl.Datetime("ms"), pl.Datetime("us"), pl.Datetime("ns")],
    ids=str,
)
def test_exact_endpoint_extremes_and_touching_ticks(dtype):
    if dtype == pl.Date:
        minimum, maximum = -(2**31), 2**31 - 1
    else:
        maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
        minimum = 0 if dtype == pl.UInt64 else -(2**63)
    physical = pl.UInt64 if dtype == pl.UInt64 else pl.Int64
    frame = pl.DataFrame(
        {
            "start": [minimum, maximum - 2, maximum - 1, maximum],
            "end": [maximum - 2, maximum - 1, maximum, maximum],
            "value": [3, 4, 5, 4],
        },
        schema={"start": physical, "end": physical, "value": pl.Int64},
    ).with_columns(pl.col("start", "end").cast(dtype))
    assert frame.select(expression()).to_series().to_list() == [False, False, True, False]
    assert_optimal(frame, frame.select(expression()).to_series())


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_exact_large_weights(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    frame = frame_from_rows([(0, 1, 0), (3, 5, 0), (3, 5, 0)]).with_columns(
        pl.Series("value", [maximum - 1, maximum - 1, 1], dtype=dtype)
    )
    assert frame.select(expression()).to_series().to_list() == [False, True, True]
    assert_optimal(frame, frame.select(expression()).to_series())


def test_real_temporal_values():
    for dtype, starts, ends in [
        (pl.Date, [date(2026, 1, d) for d in (1, 2, 3)], [date(2026, 1, d) for d in (3, 3, 4)]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((0, 30), (1, 0), (0, 45))],
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((1, 0), (1, 30), (1, 15))],
        ),
    ]:
        frame = pl.DataFrame(
            {"start": starts, "end": ends, "value": [4, 5, 6]},
            schema={"start": dtype, "end": dtype, "value": pl.Int64},
        )
        assert_optimal(frame, frame.select(expression()).to_series())


def test_filtering_before_optimization_changes_the_instance():
    frame = frame_from_rows([(0, 5, 4), (1, 4, 5), (10, 11, 8)])
    before = frame.lazy().filter(pl.col("start") != 0).filter(expression()).collect()
    after = frame.lazy().filter(expression()).filter(pl.col("start") != 0).collect()
    assert before["start"].to_list() == [10]
    assert after["start"].to_list() == [1]
