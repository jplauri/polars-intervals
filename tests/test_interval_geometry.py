"""Connectivity and elementary-cell oracles for interval geometry."""

import itertools
import random
from datetime import UTC, date, datetime

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal, assert_series_equal

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES


def frame(rows, dtype=pl.Int64):
    return pl.DataFrame(rows, schema={"start": dtype, "end": dtype}, orient="row")


def cluster_oracle(rows, touching=False):
    """Direct pair graph search, independent of coordinate sweeps."""
    labels = [None] * len(rows)
    count = 0
    for first in range(len(rows)):
        if labels[first] is not None:
            continue
        labels[first] = count
        pending = [first]
        while pending:
            index = pending.pop()
            a, b = rows[index]
            if a == b:
                continue
            for other, (c, d) in enumerate(rows):
                related = a <= d and c <= b if touching else a < d and c < b
                if c < d and related and labels[other] is None:
                    labels[other] = count
                    pending.append(other)
        count += 1
    return labels


def cells_oracle(rows, domain=None):
    """Direct membership on every elementary cell, with no production helpers."""
    points = {value for row in rows for value in row}
    if domain is not None:
        left, right = domain
        points = {left, right} | {value for value in points if left < value < right}
    result = []
    for left, right in itertools.pairwise(sorted(points)):
        covered = any(start <= left < end for start, end in rows)
        if covered == (domain is None):
            if result and result[-1][1] == left:
                result[-1] = result[-1][0], right
            else:
                result.append((left, right))
    return result


CASES = [
    [],
    [(2, 3)],
    [(2, 2)],
    [(2, 2)] * 3,
    [(1, 4), (3, 6), (5, 8), (8, 10), (12, 15)],
    [(0, 100), (1, 2), (2, 2), (3, 4), (100, 100), (101, 101)],
    [(0, 2), (2, 4), (4, 6)],
    [(10, 12), (0, 2), (1, 3)],
    [(2, 6), (2, 6), (2, 3), (3, 6), (0, 0)],
    [(5, 10), (0, 2), (20, 20), (1, 3), (5, 8)],
]


@pytest.mark.parametrize("rows", CASES)
def test_named_geometry_cases(rows):
    source = frame(rows)
    for touching in (False, True):
        actual = source.select(pi.cluster_intervals("start", "end", include_touching=touching))
        assert actual.to_series().dtype == pl.UInt32
        assert actual.to_series().to_list() == cluster_oracle(rows, touching)
    assert pi.merge_intervals(source).rows() == cells_oracle(rows)
    for domain in ((0, 16), (2, 2), (-20, -10), (5, 8), (200, 201)):
        actual = pi.interval_gaps(source, domain_start=domain[0], domain_end=domain[1])
        assert actual.rows() == cells_oracle(rows, domain)
        assert actual.schema == source.schema


def test_worked_example_and_first_occurrence_ids():
    source = frame(CASES[4])
    assert source.select(pi.cluster_intervals("start", "end")).to_series().to_list() == [
        0,
        0,
        0,
        1,
        2,
    ]
    assert source.select(
        pi.cluster_intervals("start", "end", include_touching=True)
    ).to_series().to_list() == [0, 0, 0, 0, 1]
    assert pi.merge_intervals(source).rows() == [(1, 10), (12, 15)]
    assert pi.interval_gaps(source, domain_start=0, domain_end=16).rows() == [
        (0, 1),
        (10, 12),
        (15, 16),
    ]
    reordered = frame([(10, 12), (0, 2), (1, 3)])
    assert reordered.select(pi.cluster_intervals("start", "end")).to_series().to_list() == [0, 1, 1]
    assert pi.merge_intervals(reordered).rows() == [(0, 3), (10, 12)]


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_endpoint_metadata_and_empty_schema(dtype):
    source = frame([(0, 3), (2, 5), (5, 5), (6, 8)], dtype)
    bounds = {
        "domain_start": pl.Series([0], dtype=dtype),
        "domain_end": pl.Series([10], dtype=dtype),
    }
    for data in (source, source.clear(), source.filter(pl.col("start") == pl.col("end"))):
        result = pi.merge_intervals(data)
        assert result.schema == source.schema
        assert result.select(pl.all().to_physical()).rows() == cells_oracle(
            data.select(pl.all().to_physical()).rows()
        )
        gaps = pi.interval_gaps(data, **bounds)
        assert gaps.schema == source.schema
        assert gaps.select(pl.all().to_physical()).rows() == cells_oracle(
            data.select(pl.all().to_physical()).rows(), (0, 10)
        )
        assert data.select(pi.cluster_intervals("start", "end")).to_series().dtype == pl.UInt32


@pytest.mark.parametrize(
    "dtype,left,right",
    [
        (pl.Int64, -(2**63), 2**63 - 1),
        (pl.UInt64, 2**64 - 8, 2**64 - 1),
        (pl.Int64, 2**53 + 1, 2**53 + 9),
    ],
)
def test_extreme_coordinates_are_exact(dtype, left, right):
    source = frame([(left, left + 2), (left + 2, left + 3), (right - 1, right)], dtype)
    assert pi.merge_intervals(source).rows() == [(left, left + 3), (right - 1, right)]
    assert pi.interval_gaps(source, domain_start=left, domain_end=right).rows() == [
        (left + 3, right - 1)
    ]


def test_temporal_scalars_and_physical_timestamps():
    for dtype, left, right in [
        (pl.Date, date(1970, 1, 1), date(1970, 1, 2)),
        (pl.Datetime("us"), datetime(1970, 1, 1), datetime(1970, 1, 2)),  # noqa: DTZ001
        (
            pl.Datetime("us", "UTC"),
            datetime(1970, 1, 1, tzinfo=UTC),
            datetime(1970, 1, 2, tzinfo=UTC),
        ),
    ]:
        source = frame([], dtype)
        result = pi.interval_gaps(source, domain_start=left, domain_end=right)
        assert_frame_equal(result, frame([(left, right)], dtype))
        assert result.schema == source.schema
    dtype = pl.Datetime("ns", "Europe/Helsinki")
    t = 1729990800000000001
    source = frame([(t - 2, t), (t + 1, t + 2)], dtype)
    result = pi.interval_gaps(
        source,
        domain_start=pl.Series([t - 1], dtype=dtype),
        domain_end=pl.Series([t + 3], dtype=dtype),
    )
    assert result.select(pl.all().to_physical()).rows() == [(t, t + 1), (t + 2, t + 3)]
    assert result.schema == source.schema


@pytest.mark.parametrize(
    "dtype",
    [pl.String, pl.Boolean, *INTEGER_DTYPES, pl.Date, pl.Datetime("ns", "Europe/Helsinki")],
    ids=str,
)
def test_group_key_types_and_order_before_pruning(dtype):
    values = ["b", "a", None, "b"] if dtype == pl.String else [1, 0, None, 1]
    if dtype == pl.Boolean:
        values = [True, False, None, True]
    source = frame([(8, 8), (2, 4), (20, 30), (6, 7)]).with_columns(
        pl.Series("key", values, dtype=dtype)
    )
    merged = pi.merge_intervals(source, by="key")
    assert merged.schema == {"key": dtype, "start": pl.Int64, "end": pl.Int64}
    assert merged.select("start", "end").rows() == [(6, 7), (2, 4), (20, 30)]
    gaps = pi.interval_gaps(source, by="key", domain_start=0, domain_end=5)
    assert gaps.select("start", "end").rows() == [(0, 5), (0, 2), (4, 5), (0, 5)]
    assert_series_equal(gaps["key"], source["key"].gather([0, 1, 1, 2]))
    for operation in (
        pi.merge_intervals,
        lambda df, **kw: pi.interval_gaps(df, domain_start=0, domain_end=5, **kw),
    ):
        empty = operation(source.clear(), by="key")
        assert empty.is_empty()
        assert empty.schema == merged.schema


def test_multiple_nullable_keys_custom_columns_and_payloads():
    source = (
        frame([(7, 7), (2, 4), (1, 3), (20, 30), (4, 6)])
        .rename({"start": "s", "end": "e"})
        .with_columns(
            pl.Series("key", ["b", "a", "a", None, "a"]),
            pl.Series("flag", [None, True, True, False, True], dtype=pl.Boolean),
            pl.lit("ignored").alias("start"),
            pl.lit([1, 2]).alias("end"),
            pl.lit({"x": 1}).alias("payload"),
        )
    )
    options = {"start": "s", "end": "e", "by": ["flag", "key"]}
    assert pi.merge_intervals(source, **options).rows() == [
        (True, "a", 1, 6),
        (False, None, 20, 30),
    ]
    assert pi.interval_gaps(source, **options, domain_start=0, domain_end=10).rows() == [
        (None, "b", 0, 10),
        (True, "a", 0, 1),
        (True, "a", 6, 10),
        (False, None, 0, 10),
    ]
    # Literal selector-looking names and key/endpoint aliasing must remain valid.
    literal = source.rename({"s": "*", "e": "^end$"})
    assert_frame_equal(
        pi.merge_intervals(literal, start="*", end="^end$", by="key"),
        pi.merge_intervals(source, start="s", end="e", by="key"),
    )
    same = pi.interval_gaps(source, start="s", end="s", by="s", domain_start=0, domain_end=10)
    assert same.rows() == [(7, 0, 10), (2, 0, 10), (1, 0, 10), (20, 0, 10), (4, 0, 10)]


@pytest.mark.parametrize("touching", [False, True])
def test_cluster_expression_windows_aggregation_and_filter(touching):
    rows = [(10, 12), (0, 4), (1, 3), (2, 2), (3, 7), (6, 10)]
    source = frame(rows).with_columns(pl.Series("g", ["b", "a", "b", None, "a", "a"]))
    expr = pi.cluster_intervals(
        pl.col("start") + 0, pl.col("end"), include_touching=touching
    ).alias("cluster")
    assert source.select(expr).to_series().to_list() == cluster_oracle(rows, touching)
    expected = [0] * source.height
    for key in dict.fromkeys(source["g"]):
        indices = [i for i, g in enumerate(source["g"]) if key == g]
        for index, label in zip(
            indices, cluster_oracle([rows[i] for i in indices], touching), strict=True
        ):
            expected[index] = label
    assert source.with_columns(expr.over("g"))["cluster"].to_list() == expected
    lists = source.group_by("g", maintain_order=True).agg(expr)
    assert lists["cluster"].to_list() == [
        [expected[i] for i, g in enumerate(source["g"]) if g == key] for key in lists["g"]
    ]
    assert source.filter(expr == 0).rows() == [
        row
        for row, label in zip(source.rows(), cluster_oracle(rows, touching), strict=True)
        if label == 0
    ]


@pytest.mark.parametrize("value", [0, 1, None, "true", [], pl.lit(True)])
def test_touching_requires_real_bool(value):
    with pytest.raises(TypeError):
        pi.cluster_intervals("start", "end", include_touching=value)


@pytest.mark.parametrize(
    "kwargs,error",
    [
        ({"start": 1}, TypeError),
        ({"end": pl.col("end")}, TypeError),
        ({"by": ("g",)}, TypeError),
        ({"by": [1]}, TypeError),
        ({"by": ["g", "g"]}, ValueError),
        ({"by": "start"}, ValueError),
        ({"by": "end"}, ValueError),
        ({"by": "missing"}, pl.exceptions.PolarsError),
        ({"start": "missing"}, pl.exceptions.PolarsError),
    ],
)
@pytest.mark.parametrize("lazy", [False, True])
def test_frame_invalid_arguments(kwargs, error, lazy):
    source = frame([(0, 3)]).with_columns(pl.lit("x").alias("g"))
    source = source.lazy() if lazy else source
    for operation in (
        pi.merge_intervals,
        lambda df, **kw: pi.interval_gaps(df, domain_start=0, domain_end=4, **kw),
    ):
        with pytest.raises(error):
            operation(source, **kwargs)


@pytest.mark.parametrize(
    "bound,error",
    [
        (None, (TypeError, ValueError)),
        (True, TypeError),
        (0.0, TypeError),
        (pl.Series([], dtype=pl.Int64), ValueError),
        (pl.Series([0, 1]), ValueError),
        (pl.Series([None], dtype=pl.Int64), ValueError),
        (pl.Series([0], dtype=pl.Int32), pl.exceptions.PolarsError),
        (2**64, pl.exceptions.PolarsError),
        (10, pl.exceptions.PolarsError),
    ],
)
def test_invalid_gap_bounds(bound, error):
    with pytest.raises(error):
        pi.interval_gaps(frame([(0, 3)]), domain_start=bound, domain_end=4)


@pytest.mark.parametrize(
    "dtype,bound",
    [
        (pl.Date, 0),
        (pl.Int64, date(1970, 1, 1)),
        (pl.UInt8, -1),
        (pl.Int8, 128),
        (pl.Datetime("ns"), pl.Series([0], dtype=pl.Datetime("us"))),
        (pl.Datetime("us", "UTC"), pl.Series([0], dtype=pl.Datetime("us"))),
        (pl.Datetime("ns", "UTC"), pl.Series([0], dtype=pl.Datetime("ns", "Europe/Helsinki"))),
    ],
)
def test_gap_bounds_require_exact_dtype_range_and_metadata(dtype, bound):
    with pytest.raises(pl.exceptions.PolarsError):
        pi.interval_gaps(frame([(0, 1)], dtype), domain_start=bound, domain_end=bound)


def test_missing_bounds_and_nonframes_rejected():
    with pytest.raises(TypeError):
        pi.interval_gaps(frame([]))
    for value in (None, [], pl.Series([1])):
        with pytest.raises(TypeError):
            pi.merge_intervals(value)
        with pytest.raises(TypeError):
            pi.interval_gaps(value, domain_start=0, domain_end=1)


@pytest.mark.parametrize(
    "dtype",
    [pl.Float64, pl.Int128, pl.Categorical, pl.List(pl.Int64), pl.Object, pl.Duration("us")],
    ids=str,
)
def test_unsupported_group_types_are_rejected_before_ffi(dtype):
    source = frame([(0, 3)]).with_columns(pl.Series("g", [None], dtype=dtype))
    for data in (source, source.lazy()):
        with pytest.raises(pl.exceptions.PolarsError):
            pi.merge_intervals(data, by="g")
        with pytest.raises(pl.exceptions.PolarsError):
            pi.interval_gaps(data, by="g", domain_start=0, domain_end=5)


@pytest.mark.parametrize("domain", [(0, 0), (100, 200)])
def test_validation_precedes_grouping_pruning_and_clipping(domain):
    source = frame([(0, 2), (30, 30), (10, 9)]).with_columns(pl.Series("g", ["b", "a", "b"]))
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        pi.merge_intervals(source, by="g")
    with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
        pi.interval_gaps(source, by="g", domain_start=domain[0], domain_end=domain[1])


def test_random_original_problem_oracles_and_grouped_lazy_queries():
    rng = random.Random(90321)
    for _ in range(36):
        rows = [
            tuple(sorted((rng.randrange(-8, 9), rng.randrange(-8, 9))))
            for _ in range(rng.randrange(25))
        ]
        keys = [rng.choice([None, "a", "b"]) for _ in rows]
        source = frame(rows).with_columns(pl.Series("g", keys, dtype=pl.String))
        left, right = sorted((rng.randrange(-10, 11), rng.randrange(-10, 11)))
        for touching in (False, True):
            actual = (
                source.lazy()
                .select(pi.cluster_intervals("start", "end", include_touching=touching))
                .collect()
                .to_series()
            )
            assert actual.to_list() == cluster_oracle(rows, touching)
        for by in (None, [], "g"):
            groups = (
                [(None, rows)]
                if not by
                else [
                    (key, [row for row, group in zip(rows, keys, strict=True) if group == key])
                    for key in dict.fromkeys(keys)
                ]
            )
            for gap in (False, True):
                options = {"by": by}
                if gap:
                    options.update(domain_start=left, domain_end=right)
                operation = pi.interval_gaps if gap else pi.merge_intervals
                expected = [
                    ((*(() if not by else (key,)), *segment))
                    for key, group in groups
                    for segment in cells_oracle(group, (left, right) if gap else None)
                ]
                eager = operation(source, **options)
                assert eager.rows() == expected
                assert_frame_equal(eager, operation(source.lazy(), **options).collect())
