"""Public profile tests use direct interval membership, never signed events."""

import itertools
import random
from datetime import UTC, date, datetime

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES


def oracle(rows, domain=None, include_zero=False):
    """Small independent O(n*u) oracle with Python arbitrary-precision loads."""
    nonempty = [(s, e, q) for s, e, q in rows if s < e]
    if domain is None:
        if not nonempty:
            return []
        domain = min(s for s, _, _ in nonempty), max(e for _, e, _ in nonempty)
    left, right = domain
    points = sorted({left, right} | {x for s, e, _ in nonempty for x in (s, e) if left < x < right})
    result = []
    for a, b in itertools.pairwise(points):
        q = sum(q for s, e, q in rows if s <= a < e)
        if a == b or (not include_zero and q == 0):
            continue
        if result and result[-1][1] == a and result[-1][2] == q:
            result[-1] = result[-1][0], b, q
        else:
            result.append((a, b, q))
    return result


def frame(rows):
    return pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "q": pl.Int64}, orient="row"
    )


def test_examples_defaults_custom_names_and_input_unchanged():
    df = pl.DataFrame(
        {"s": [0, 2, 5], "e": [4, 5, 7], "demand": [2, 3, 3], "weight": [-9] * 3, "load": [100] * 3}
    )
    before = df.clone()
    expected = [(0, 2, 1), (2, 4, 2), (4, 7, 1)]
    for options in ({}, {"weight": None}):
        result = pi.coverage_profile(df, start="s", end="e", **options)
        assert result.rows() == expected
        assert result.schema == {"start": pl.Int64, "end": pl.Int64, "load": pl.Int128}
    weighted = pi.coverage_profile(df, start="s", end="e", weight="demand")
    assert weighted.rows() == [(0, 2, 2), (2, 4, 5), (4, 7, 3)]
    assert pi.coverage_profile(
        df, start="s", end="e", weight="demand", domain_start=-1, domain_end=8, include_zero=True
    ).rows() == [(-1, 0, 0), *weighted.rows(), (7, 8, 0)]
    assert_frame_equal(df, before)
    assert_frame_equal(pi.coverage_profile(df, start="s", end="e", by=[]), result)


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_endpoint_types_and_empty_schema(dtype):
    df = pl.DataFrame({"start": [0, 2], "end": [2, 4]}).cast(dtype)
    result = pi.coverage_profile(df)
    assert result.schema == {"start": dtype, "end": dtype, "load": pl.Int128}
    assert result.select(pl.col("start", "end").to_physical(), "load").rows() == [(0, 4, 1)]
    assert pi.coverage_profile(df.clear()).schema == result.schema
    assert result.null_count().row(0) == (0, 0, 0)


@pytest.mark.parametrize("dtype", INTEGER_DTYPES, ids=str)
def test_weight_types(dtype):
    df = frame([(0, 3, 0), (1, 2, 2), (1, 2, 3)]).with_columns(pl.col("q").cast(dtype))
    assert pi.coverage_profile(df, weight="q", include_zero=True).rows() == [
        (0, 1, 0),
        (1, 2, 5),
        (2, 3, 0),
    ]


@pytest.mark.parametrize("rows", [[], [(9, 9, 4)], [(9, 9, 0), (-9, -9, 3)]])
def test_no_inferred_domain(rows):
    df = frame(rows)
    for zero in (False, True):
        assert pi.coverage_profile(df, weight="q", include_zero=zero).is_empty()
        assert pi.coverage_profile(
            df, weight="q", domain_start=0, domain_end=5, include_zero=zero
        ).rows() == ([(0, 5, 0)] if zero else [])


def test_zero_hull_and_gap_not_coalesced():
    df = frame([(0, 100, 0), (10, 20, 2), (-100, -100, 99), (200, 200, 99)])
    assert pi.coverage_profile(df, weight="q", include_zero=True).rows() == [
        (0, 10, 0),
        (10, 20, 2),
        (20, 100, 0),
    ]
    assert pi.coverage_profile(df.head(1), weight="q", include_zero=True).rows() == [(0, 100, 0)]
    df = frame([(0, 2, 5), (4, 6, 5)])
    assert pi.coverage_profile(df, weight="q").rows() == [(0, 2, 5), (4, 6, 5)]


@pytest.mark.parametrize("bounds", [(-4, -1), (8, 9), (1, 5), (2, 4), (0, 6), (2, 2)])
@pytest.mark.parametrize("zero", [False, True])
def test_domains(bounds, zero):
    rows = [(0, 2, 5), (4, 6, 5)]
    assert pi.coverage_profile(
        frame(rows), weight="q", domain_start=bounds[0], domain_end=bounds[1], include_zero=zero
    ).rows() == oracle(rows, bounds, zero)


def test_exact_uint64_totals_and_checked_narrowing():
    maximum = 2**64 - 1
    df = pl.DataFrame(
        {
            "start": [2**53 + 1, 2**53 + 1],
            "end": [2**53 + 3] * 2,
            "q": pl.Series([maximum, maximum], dtype=pl.UInt64),
        }
    )
    result = pi.coverage_profile(df, weight="q")
    assert result.rows() == [(2**53 + 1, 2**53 + 3, 2 * maximum)]
    with pytest.raises(pl.exceptions.InvalidOperationError):
        result.with_columns(pl.col("load").cast(pl.UInt64, strict=True))
    bounded = pi.coverage_profile(frame([(0, 2, 3), (1, 3, 4)]), weight="q")
    narrowed = bounded.with_columns(pl.col("load").cast(pl.UInt64, strict=True))
    assert_frame_equal(pi.coverage_profile(narrowed, weight="load"), bounded)
    assert narrowed.filter(pi.max_weight_clique("start", "end", weight="load"))["load"].sum() == 7


def test_temporal_bounds_and_exact_offset_transition():
    for dtype, left, right in [
        (pl.Date, date(2024, 1, 1), date(2024, 1, 3)),
        (pl.Datetime("us"), datetime(2024, 1, 1), datetime(2024, 1, 3)),  # noqa: DTZ001 - test naive timestamps
        (
            pl.Datetime("us", "UTC"),
            datetime(2024, 1, 1, tzinfo=UTC),
            datetime(2024, 1, 3, tzinfo=UTC),
        ),
    ]:
        df = pl.DataFrame(
            {"start": pl.Series([left], dtype=dtype), "end": pl.Series([right], dtype=dtype)}
        )
        assert_frame_equal(
            pi.coverage_profile(df), pi.coverage_profile(df, domain_start=left, domain_end=right)
        )
    # UTC physical nanoseconds surrounding Helsinki's autumn offset transition.
    dtype = pl.Datetime("ns", "Europe/Helsinki")
    t = 1729990800000000001
    df = pl.DataFrame({"start": [t - 2, t], "end": [t, t + 2]}).cast(dtype)
    result = pi.coverage_profile(
        df, domain_start=pl.Series([t - 1], dtype=dtype), domain_end=pl.Series([t + 1], dtype=dtype)
    )
    assert result.select(pl.col("start", "end").to_physical(), "load").rows() == [(t - 1, t + 1, 1)]
    assert result.schema["start"] == dtype


@pytest.mark.parametrize(
    "kwargs,error",
    [
        ({"start": 1}, TypeError),
        ({"end": pl.col("end")}, TypeError),
        ({"weight": 1}, TypeError),
        ({"include_zero": 1}, TypeError),
        ({"include_zero": None}, TypeError),
        ({"by": ("g",)}, TypeError),
        ({"by": [1]}, TypeError),
        ({"by": ["g", "g"]}, ValueError),
        ({"by": "start"}, ValueError),
        ({"by": "end"}, ValueError),
        ({"by": "load"}, ValueError),
        ({"start": "missing"}, pl.exceptions.PolarsError),
        ({"weight": "missing"}, pl.exceptions.PolarsError),
        ({"by": "missing"}, pl.exceptions.PolarsError),
        ({"domain_start": 0}, ValueError),
        ({"domain_start": True, "domain_end": 4}, TypeError),
        ({"domain_start": 0.0, "domain_end": 4}, TypeError),
        ({"domain_start": pl.Series([], dtype=pl.Int64), "domain_end": 4}, ValueError),
        ({"domain_start": pl.Series([0, 1]), "domain_end": 4}, ValueError),
        ({"domain_start": pl.Series([None], dtype=pl.Int64), "domain_end": 4}, ValueError),
        (
            {"domain_start": pl.Series([0], dtype=pl.Int32), "domain_end": 4},
            pl.exceptions.PolarsError,
        ),
        ({"domain_start": 2**64, "domain_end": 2**64}, pl.exceptions.PolarsError),
        ({"domain_start": 4, "domain_end": 0}, pl.exceptions.PolarsError),
    ],
)
def test_invalid_options(kwargs, error):
    with pytest.raises(error):
        pi.coverage_profile(frame([(0, 3, 1)]).with_columns(pl.lit("x").alias("g")), **kwargs)


def test_nonframe_rejected():
    for value in ([], None, pl.Series([1])):
        with pytest.raises(TypeError, match="DataFrame|LazyFrame"):
            pi.coverage_profile(value)


def test_native_series_length_errors_do_not_panic():
    from polars_intervals import _internal

    starts, ends, weights = pl.Series([0, 1]), pl.Series([2, 3]), pl.Series([1, 1])
    for s, e, q, keys in [
        (starts, ends.head(1), weights, []),
        (starts, ends, weights.head(1), []),
        (starts, ends, weights, [pl.Series("g", [1])]),
    ]:
        with pytest.raises(pl.exceptions.ShapeError):
            _internal.coverage_profile(s, e, q, keys, None, False)


@pytest.mark.parametrize(
    "dtype,bound",
    [
        (pl.Date, 0),
        (pl.Int64, date(1970, 1, 1)),
        (pl.Datetime("ns"), pl.Series([0], dtype=pl.Datetime("us"))),
        (pl.Datetime("us", "UTC"), pl.Series([0], dtype=pl.Datetime("us"))),
        (pl.Datetime("ns", "UTC"), pl.Series([0], dtype=pl.Datetime("ns", "Europe/Helsinki"))),
    ],
)
def test_bounds_require_exact_temporal_metadata(dtype, bound):
    df = pl.DataFrame({"start": [0], "end": [1]}).cast(dtype)
    with pytest.raises(pl.exceptions.PolarsError):
        pi.coverage_profile(df, domain_start=bound, domain_end=bound)


@pytest.mark.parametrize("column", ["start", "end"])
@pytest.mark.parametrize("bounds", [(0, 0), (10, 20)])
def test_null_endpoints_are_invalid_before_zero_output(column, bounds):
    df = frame([(0, 0, 0)]).with_columns(pl.lit(None, dtype=pl.Int64).alias(column))
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        pi.coverage_profile(df, weight="q", domain_start=bounds[0], domain_end=bounds[1])


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float64,
        pl.Boolean,
        pl.Int128,
        pl.UInt128,
        pl.Decimal(20, 0),
        pl.Date,
        pl.Datetime("us"),
        pl.String,
        pl.List(pl.Int64),
        pl.Categorical,
        pl.Object,
    ],
)
def test_unsupported_weights_before_ffi(dtype):
    df = frame([]).with_columns(pl.Series("q", [], dtype=dtype))
    with pytest.raises(pl.exceptions.InvalidOperationError):
        pi.coverage_profile(df, weight="q", domain_start=0, domain_end=0)


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float64,
        pl.Int128,
        pl.UInt128,
        pl.Decimal(20, 0),
        pl.Time,
        pl.Duration("us"),
        pl.List(pl.Int64),
        pl.Categorical,
        pl.Object,
    ],
)
def test_unsupported_group_keys_before_ffi(dtype):
    df = frame([]).with_columns(pl.Series("g", [], dtype=dtype))
    with pytest.raises(pl.exceptions.InvalidOperationError):
        pi.coverage_profile(df, by="g")


@pytest.mark.parametrize("bounds", [(0, 0), (50, 60)])
def test_validate_irrelevant_rows_original_indices(bounds):
    for rows, match in [([(0, 1, 1), (9, 9, -1)], "index 1"), ([(0, 1, 1), (9, 8, 0)], "index 1")]:
        df = frame(rows).with_columns(pl.Series("g", ["a", "b"]))
        with pytest.raises(pl.exceptions.ComputeError, match=match):
            pi.coverage_profile(
                df, weight="q", by="g", domain_start=bounds[0], domain_end=bounds[1]
            )
    with pytest.raises(pl.exceptions.ComputeError, match="null"):
        pi.coverage_profile(frame([(0, 0, None)]), weight="q", domain_start=0, domain_end=0)


@pytest.mark.parametrize(
    "dtype", [pl.String, pl.Boolean, *INTEGER_DTYPES, pl.Date, pl.Datetime("ns", "UTC")], ids=str
)
def test_group_key_dtype_null_equality_and_empty_schema(dtype):
    values = ["b", None, "a", "b"] if dtype == pl.String else [1, None, 0, 1]
    df = frame([(0, 2, 1), (0, 1, 2), (0, 3, 3), (2, 4, 1)]).with_columns(
        pl.Series("g", values).cast(dtype)
    )
    result = pi.coverage_profile(df, weight="q", by="g")
    assert result.columns == ["g", "start", "end", "load"]
    assert result["g"].equals(df["g"].head(3))
    assert result.select("start", "end", "load").rows() == [(0, 4, 1), (0, 1, 2), (0, 3, 3)]
    assert (
        pi.coverage_profile(
            df.clear(), by="g", domain_start=0, domain_end=5, include_zero=True
        ).schema
        == result.schema
    )
    assert pi.coverage_profile(df, by="g", domain_start=0, domain_end=0).schema == result.schema


def test_multiple_group_keys_and_observed_empty_groups():
    df = frame([(0, 0, 4), (0, 3, 0), (0, 3, 2), (0, 3, 2)]).with_columns(
        pl.Series("g", [None, "b", "a", "a"]), pl.Series("h", [1, 2, 2, 3])
    )
    result = pi.coverage_profile(
        df, weight="q", by=["h", "g"], domain_start=-1, domain_end=4, include_zero=True
    )
    assert result.columns == ["h", "g", "start", "end", "load"]
    assert result.rows()[:2] == [(1, None, -1, 4, 0), (2, "b", -1, 4, 0)]
    assert result.rows()[2:] == [
        (2, "a", -1, 0, 0),
        (2, "a", 0, 3, 2),
        (2, "a", 3, 4, 0),
        (3, "a", -1, 0, 0),
        (3, "a", 0, 3, 2),
        (3, "a", 3, 4, 0),
    ]
    # Source and group columns may be the same when the output name is unreserved.
    assert pi.coverage_profile(
        df, start="h", end="h", by="h", domain_start=0, domain_end=4, include_zero=True
    ).rows() == [(1, 0, 4, 0), (2, 0, 4, 0), (3, 0, 4, 0)]


def test_chunks_permutations_group_independence_and_python_oracle():
    rng = random.Random(2819)
    for _ in range(60):
        rows = [
            (*sorted((rng.randrange(-6, 7), rng.randrange(-6, 7))), rng.randrange(5))
            for _ in range(rng.randrange(25))
        ]
        df = frame(rows).with_columns(
            pl.Series("g", [rng.choice([None, "a", "b"]) for _ in rows], dtype=pl.String)
        )
        columns = [
            pl.concat([s.slice(i, step) for i in range(0, len(s), step)], rechunk=False)
            if len(s)
            else s
            for step, s in enumerate(df, start=2)
        ]
        chunked = pl.DataFrame(columns)
        for bounds in (None, (-3, 4), (0, 0)):
            for zero in (False, True):
                options = {"include_zero": zero}
                if bounds:
                    options.update(domain_start=bounds[0], domain_end=bounds[1])
                got = pi.coverage_profile(chunked, weight="q", **options)
                assert got.rows() == oracle(rows, bounds, zero)
                assert_frame_equal(got, pi.coverage_profile(df.reverse(), weight="q", **options))
                assert_frame_equal(
                    pi.coverage_profile(df, **options),
                    pi.coverage_profile(
                        df.with_columns(pl.lit(1).alias("ones")), weight="ones", **options
                    ),
                )
                grouped = pi.coverage_profile(chunked, weight="q", by="g", **options)
                expected = []
                for key in dict.fromkeys(df["g"].to_list()):
                    subrows = [(s, e, q) for s, e, q, g in df.iter_rows() if g == key]
                    independent = pi.coverage_profile(frame(subrows), weight="q", **options)
                    assert independent.rows() == oracle(subrows, bounds, zero)
                    expected.extend((key, *segment) for segment in oracle(subrows, bounds, zero))
                assert grouped.rows() == expected
                assert_frame_equal(
                    grouped.sort(["g", "start"]),
                    pi.coverage_profile(df.reverse(), weight="q", by="g", **options).sort(
                        ["g", "start"]
                    ),
                )


def test_public_metamorphic_partition_split_scaling_and_ticks():
    rng = random.Random(8524)
    for _ in range(40):
        rows = [
            (*sorted((rng.randrange(-5, 6), rng.randrange(-5, 6))), rng.randrange(5))
            for _ in range(15)
        ]
        options = {"domain_start": -5, "domain_end": 5, "include_zero": True, "weight": "q"}
        result = pi.coverage_profile(frame(rows), **options)
        assert result.rows() == oracle(rows, (-5, 5), True)
        # A second oracle has no breakpoint construction.
        for tick in range(-5, 5):
            assert sum(q for s, e, q in result.iter_rows() if s <= tick < e) == sum(
                q for s, e, q in rows if s <= tick < e
            )
        split = [
            (a, b, q)
            for s, e, q in rows
            for a, b in ([(s, (s + e) // 2), ((s + e) // 2, e)] if e - s > 1 else [(s, e)])
        ]
        assert_frame_equal(result, pi.coverage_profile(frame(split), **options))
        doubled = pi.coverage_profile(frame(rows + rows), **options)
        assert_frame_equal(doubled, result.with_columns(pl.col("load") * 2))
        parts = (
            pi.coverage_profile(frame(rows[:7]), **options).rows()
            + pi.coverage_profile(frame(rows[7:]), **options).rows()
        )
        assert oracle(parts, (-5, 5), True) == result.rows()
        assert sum(q * (e - s) for s, e, q in result.iter_rows()) == sum(
            q * (e - s) for s, e, q in rows
        )
        assert_frame_equal(
            pi.coverage_profile(frame(rows + [(0, 0, 99), (-8, 8, 0)]), **options), result
        )
