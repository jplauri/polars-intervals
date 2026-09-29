"""Compiled expression tests with an independent original-graph subset oracle."""

from datetime import UTC, date, datetime
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES


def expression(cost="price"):
    return pi.minimum_cost_dominating_set("start", "end", cost=cost).alias("selected")


def frame_from_rows(rows):
    return pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "price": pl.Int64}, orient="row"
    )


def assert_optimal(frame, mask, *, units=False):
    starts = frame["start"].to_physical().to_list()
    ends = frame["end"].to_physical().to_list()
    costs = [1] * len(frame) if units else frame["price"].to_list()
    assert mask.dtype == pl.Boolean
    assert mask.null_count() == 0
    assert len(mask) == len(frame)

    def dominates(indices):
        return all(
            i in indices
            or any(
                starts[i] < ends[i]
                and starts[j] < ends[j]
                and starts[i] < ends[j]
                and starts[j] < ends[i]
                for j in indices
            )
            for i in range(len(frame))
        )

    optimum = None
    for bits in range(1 << len(frame)):
        indices = {i for i in range(len(frame)) if bits & (1 << i)}
        if dominates(indices):
            objective = (sum(costs[i] for i in indices), len(indices))
            optimum = objective if optimum is None else min(optimum, objective)
    chosen = {i for i, selected in enumerate(mask) if selected}
    assert dominates(chosen)
    assert (sum(costs[i] for i in chosen), len(chosen)) == optimum
    assert all(not dominates(chosen - {i}) for i in chosen)
    return optimum


@pytest.mark.parametrize(
    "rows,expected",
    [
        ([], []),
        ([(1, 3, 0)], [True]),
        ([(1, 3, 7)], [True]),
        ([(1, 1, 0)], [True]),
        ([(2, 2, 0), (2, 2, 5), (0, 0, 0)], [True, True, True]),
        ([(0, 5, 1), (2, 2, 0), (5, 5, 2)], [True, True, True]),
        ([(0, 2, 1), (2, 4, 1)], [True, True]),
        ([(0, 5, 9), (0, 5, 1), (0, 5, 0)], [False, False, True]),
        ([(0, 4, 1), (3, 7, 1), (6, 10, 1)], [False, True, False]),
        ([(0, 4, 1), (3, 7, 10), (6, 10, 1)], [True, False, True]),
        ([(0, 4, 1), (3, 7, 2), (6, 10, 1)], [False, True, False]),
        ([(0, 4, 0), (3, 7, 0), (6, 10, 0)], [False, True, False]),
        ([(0, 2, 100), (1, 4, 1), (3, 6, 1), (5, 7, 100)], [False, True, True, False]),
        ([(0, 10, 1), (1, 2, 1), (4, 5, 1), (7, 8, 1)], [True, False, False, False]),
        ([(0, 10, 10), (1, 2, 1), (4, 5, 1), (7, 8, 1)], [False, True, True, True]),
        ([(0, 10, 1), (1, 2, 100), (7, 8, 100)], [True, False, False]),
        ([(20, 22, 4), (0, 10, 1), (21, 23, 0), (1, 2, 5)], [False, True, True, False]),
        ([(0, 10, 3), (1, 9, 1), (2, 8, 2)], [False, True, False]),
        ([(0, 10, 3), (1, 9, 2), (2, 8, 1)], [False, False, True]),
        ([(0, 10, 1), (1, 9, 2), (2, 8, 3)], [True, False, False]),
        ([(0, 3, 0), (1, 4, 0), (2, 5, 0), (10, 11, 0)], None),
    ],
)
def test_semantics_contexts_and_determinism(rows, expected):
    frame = frame_from_rows(rows)
    mask = frame.select(expression()).to_series()
    if expected is not None:
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
    explicit = frame.with_columns(pl.lit(1).alias("price")).select(expression()).to_series()
    assert unit.sum() == explicit.sum()
    assert_optimal(frame.with_columns(pl.lit(1).alias("price")), explicit)


def test_units_ignore_named_cost_column_and_support_expressions():
    frame = frame_from_rows([(0, 4, 1), (3, 7, 10), (6, 10, 1)]).with_columns(
        pl.col("price").alias("cost"), pl.lit(1).alias("ones")
    )
    for expr in (
        pi.minimum_cost_dominating_set("start", "end"),
        pi.minimum_cost_dominating_set("start", "end", cost=None),
        pi.minimum_cost_dominating_set(pl.col("start"), pl.col("end"), cost="ones"),
    ):
        assert frame.select(expr).to_series().to_list() == [False, True, False]
    assert frame.select(expression()).to_series().to_list() == [True, False, True]
    assert frame.select(
        pi.minimum_cost_dominating_set(
            pl.col("start") + 100, pl.col("end") + 100, cost=pl.col("price") * 2
        )
    ).to_series().to_list() == [True, False, True]
    assert "minimum_cost_dominating_set" in pi.__all__


def test_already_dominated_vertices_remain_eligible_representatives():
    frame = frame_from_rows([(0, 2, 1), (1, 5, 1), (3, 9, 1), (5, 6, 1), (8, 10, 1)])
    for cost in (None, "price"):
        mask = frame.select(expression(cost)).to_series()
        assert assert_optimal(frame, mask, units=cost is None) == (2, 2)


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_endpoint_types_schema_and_unit_equivalence(dtype):
    frame = frame_from_rows([(0, 4, 1), (3, 7, 1), (6, 10, 1), (2, 2, 1)]).with_columns(
        pl.col("start", "end").cast(dtype)
    )
    for subset in (frame, frame.slice(1, 3), frame.head(0), frame.head(1)):
        for cost in (None, "price"):
            query = subset.lazy().with_columns(expression(cost))
            assert query.collect_schema()["selected"] == pl.Boolean
            assert_optimal(subset, query.collect()["selected"], units=cost is None)


@pytest.mark.parametrize("dtype", INTEGER_DTYPES, ids=str)
def test_cost_types_include_zeros(dtype):
    frame = frame_from_rows([(0, 4, 1), (3, 7, 10), (6, 10, 0), (7, 7, 0)]).with_columns(
        pl.col("price").cast(dtype)
    )
    mask = frame.select(expression()).to_series()
    assert mask.to_list() == [True, False, True, True]
    assert_optimal(frame, mask)


@pytest.mark.parametrize("cost", [None, "price"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_mismatched_chunk_boundaries_and_global_reconstruction(cost, engine):
    frame = frame_from_rows([(1, 2, 10), (4, 5, 10), (0, 10, 1), (7, 8, 10), (7, 7, 0), (7, 7, 1)])
    chunked = pl.DataFrame(
        [
            pl.concat([frame[name][:cut], frame[name][cut:]], rechunk=False)
            for name, cut in (("start", 2), ("end", 4), ("price", 3))
        ]
    )
    assert [series.chunk_lengths() for series in chunked] == [[2, 4], [4, 2], [3, 3]]
    for subset in (chunked, chunked.slice(1, 4), chunked.head(0)):
        mask = subset.lazy().select(expression(cost)).collect(engine=engine).to_series()
        assert_optimal(subset, mask, units=cost is None)
        assert mask.equals(subset.rechunk().select(expression(cost)).to_series())
    assert chunked.select(expression(cost)).to_series().to_list() == [
        False,
        False,
        True,
        False,
        True,
        True,
    ]


@pytest.mark.parametrize("cost", [None, "price"])
def test_streaming_parquet_row_groups_form_one_domination_instance(tmp_path, cost):
    # The unique optimal hub occurs in the final row group and dominates all
    # preceding leaves. Optimizing independent reader batches cannot find it.
    leaves = 4096
    frame = frame_from_rows([(2 * i, 2 * i + 1, 10) for i in range(leaves)] + [(0, 2 * leaves, 1)])
    path = tmp_path / "star.parquet"
    frame.write_parquet(path, row_group_size=64)
    mask = pl.scan_parquet(path).select(expression(cost)).collect(engine="streaming").to_series()
    assert mask.dtype == pl.Boolean and mask.null_count() == 0
    assert mask.to_list() == [False] * leaves + [True]
    assert mask.equals(frame.select(expression(cost)).to_series())


@pytest.mark.parametrize(
    "dtype", [pl.Int64, pl.Date, pl.Datetime("ns", "Europe/Helsinki")], ids=str
)
@pytest.mark.parametrize("cost", [None, "price"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_interleaved_whole_groups_and_aggregation(dtype, cost, engine):
    frame = frame_from_rows(
        [
            (0, 4, 1),
            (5, 5, 0),
            (0, 4, 0),
            (3, 7, 10),
            (5, 5, 2),
            (3, 7, 0),
            (6, 10, 1),
            (6, 10, 0),
            (8, 9, 0),
        ]
    ).with_columns(
        pl.col("start", "end").cast(dtype),
        pl.Series("group", ["a", "b", "c", "a", "b", "c", "a", "c", "d"]),
    )
    frame = pl.concat([frame[:3], frame[3:6], frame[6:]], rechunk=False)
    window = frame.lazy().with_columns(expression(cost).over("group")).collect(engine=engine)
    assert window.equals(frame.with_columns(expression(cost).over("group")))
    query = frame.lazy().group_by("group", maintain_order=True).agg(expression(cost))
    assert query.collect_schema()["selected"] == pl.List(pl.Boolean)
    for group, masks in query.collect(engine=engine).iter_rows():
        subset = window.filter(pl.col("group") == group)
        independent = subset.select(expression(cost)).to_series()
        assert_optimal(subset, independent, units=cost is None)
        assert independent.to_list() == masks == subset["selected"].to_list()


def test_python_subset_oracle_random_instances_groups_and_permutations():
    rng = Random(180917)
    for _ in range(60):
        rows = []
        for _ in range(rng.randrange(10)):
            start, end = sorted([rng.randrange(-4, 6), rng.randrange(-4, 6)])
            rows.append((start, end, rng.choice([0, 0, 1, 2, 5, 10])))
        frame = frame_from_rows(rows)
        for cost in (None, "price"):
            mask = frame.select(expression(cost)).to_series()
            objective = assert_optimal(frame, mask, units=cost is None)
            assert mask.equals(
                frame.lazy().select(expression(cost)).collect(engine="streaming").to_series()
            )
            shuffled = frame.sample(fraction=1, shuffle=True, seed=17)
            assert objective == assert_optimal(
                shuffled, shuffled.select(expression(cost)).to_series(), units=cost is None
            )
        grouped = frame.with_columns(pl.Series("group", [i % 3 for i in range(len(rows))]))
        for subset in grouped.with_columns(expression().over("group")).partition_by("group"):
            assert_optimal(subset, subset["selected"])


@pytest.mark.parametrize("cost", [None, "price"])
def test_reversed_rows_before_empty_only_containment_or_duplicate_fast_paths(cost):
    for rows in (
        [(0, 0, 0), (1, 1, 0), (5, 4, 0)],
        [(0, 10, 1), (0, 10, 2), (5, 4, 0)],
    ):
        with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
            frame_from_rows(rows).select(expression(cost))


def test_negative_costs_before_empty_only_containment_or_duplicate_fast_paths():
    for rows in (
        [(0, 0, 0), (1, 1, 0), (5, 5, -1)],
        [(0, 10, 1), (1, 2, 0), (0, 10, -(2**63))],
    ):
        with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
            frame_from_rows(rows).select(expression())


@pytest.mark.parametrize("negative", [False, True])
def test_invalid_group_error_uses_original_group_order(negative):
    bad = (5, 5, -1) if negative else (5, 4, 0)
    frame = frame_from_rows([(1, 1, 0), (0, 0, 0), (1, 1, 0), bad]).with_columns(
        pl.Series("group", ["b", "a", "a", "a"])
    )
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        frame.select(expression().over("group"))


@pytest.mark.parametrize("argument", ["start", "end", "price"])
def test_null_in_each_argument(argument):
    frame = frame_from_rows([(0, 0, 0), (1, 2, 0)]).with_columns(
        pl.Series(argument, [0, None], dtype=pl.Int64)
    )
    message = "null costs" if argument == "price" else "null endpoints"
    with pytest.raises(pl.exceptions.PolarsError, match=message):
        frame.select(expression())
    if argument != "price":
        with pytest.raises(pl.exceptions.PolarsError, match=message):
            frame.select(expression(None))


def test_none_is_configuration_but_null_expression_is_invalid():
    frame = frame_from_rows([(1, 1, 0)])
    assert frame.select(expression(None)).to_series().to_list() == [True]
    with pytest.raises(pl.exceptions.PolarsError, match="null costs"):
        frame.select(
            pi.minimum_cost_dominating_set("start", "end", cost=pl.lit(None, dtype=pl.Int64))
        )


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
@pytest.mark.parametrize("cost", [None, "price"])
def test_matching_logical_dtypes_at_plan_time(start_dtype, end_dtype, cost):
    for values in ([0], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series(values, dtype=start_dtype),
                "end": pl.Series(values, dtype=end_dtype),
                "price": pl.Series(values, dtype=pl.Int64),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
            frame.lazy().select(expression(cost)).collect_schema()
        with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
            frame.select(expression(cost))


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
def test_unsupported_endpoints_are_safe_errors_at_schema_and_runtime(dtype):
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
def test_unsupported_costs_at_schema_and_runtime(dtype):
    for values in ([None], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series([0] * len(values), dtype=pl.Int64),
                "end": pl.Series([0] * len(values), dtype=pl.Int64),
                "price": pl.Series(values, dtype=dtype),
            }
        )
        with pytest.raises(pl.exceptions.PolarsError, match="integer cost dtype"):
            frame.lazy().select(expression()).collect_schema()
        with pytest.raises(pl.exceptions.PolarsError, match="integer cost dtype"):
            frame.select(expression())


@pytest.mark.parametrize("argument", ["start", "end", "cost"])
@pytest.mark.parametrize("scalar", [False, True])
def test_no_scalar_or_short_expression_broadcasting(argument, scalar):
    args = {"start": pl.col("start"), "end": pl.col("end"), "cost": pl.col("price")}
    args[argument] = pl.lit(1, dtype=pl.Int64) if scalar else args[argument].head(1)
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame_from_rows([(0, 2, 1), (1, 3, 2)]).select(pi.minimum_cost_dominating_set(**args))


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
            "price": [3, 4, 5, 0],
        },
        schema={"start": physical, "end": physical, "price": pl.Int64},
    ).with_columns(pl.col("start", "end").cast(dtype))
    assert frame.select(expression()).to_series().to_list() == [True, True, True, True]
    assert_optimal(frame, frame.select(expression()).to_series())


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_exact_large_costs_and_objective_exceeding_u64(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    frame = frame_from_rows([(0, 2, 0), (0, 2, 0), (3, 4, 0)]).with_columns(
        pl.Series("price", [maximum, maximum - 1, maximum], dtype=dtype)
    )
    assert frame.select(expression()).to_series().to_list() == [False, True, True]
    assert assert_optimal(frame, frame.select(expression()).to_series()) == (2 * maximum - 1, 2)


def test_real_temporal_values():
    for dtype, starts, ends in [
        (pl.Date, [date(2026, 1, d) for d in (1, 2, 3)], [date(2026, 1, d) for d in (3, 4, 5)]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((0, 30), (0, 45), (1, 0))],
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((1, 0), (1, 15), (1, 30))],
        ),
    ]:
        frame = pl.DataFrame(
            {"start": starts, "end": ends, "price": [1, 10, 1]},
            schema={"start": dtype, "end": dtype, "price": pl.Int64},
        )
        assert frame.select(expression()).to_series().to_list() == [True, False, True]
        assert_optimal(frame, frame.select(expression()).to_series())


def test_filtering_before_optimization_changes_both_demands_and_candidates():
    frame = frame_from_rows([(0, 4, 1), (3, 7, 10), (6, 10, 1)])
    before = frame.lazy().filter(pl.col("start") != 3).filter(expression(None)).collect()
    after = frame.lazy().filter(expression(None)).filter(pl.col("start") != 3).collect()
    assert before["start"].to_list() == [0, 6]
    assert after.is_empty()
