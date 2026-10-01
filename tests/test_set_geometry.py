"""Two-source set geometry against original-row elementary-cell oracles."""

import random
from itertools import pairwise

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES

OPERATIONS = [pi.subtract_intervals, pi.intersect_intervals]


def frame(rows, dtype=pl.Int64):
    return pl.DataFrame(rows, schema={"start": dtype, "end": dtype}, orient="row")


def cells(left, right, intersection):
    """Inspect original rows at every distinct cell, without production merging."""
    boundaries = sorted({x for row in [*left, *right] for x in row})
    output = []
    for start, end in pairwise(boundaries):
        in_left = any(s <= start < e for s, e in left)
        in_right = any(s <= start < e for s, e in right)
        if in_left and (in_right if intersection else not in_right):
            if output and output[-1][1] == start:
                output[-1] = (output[-1][0], end)
            else:
                output.append((start, end))
    return output


def grouped_oracle(left, right, keys, operation):
    left_groups, right_groups = {}, {}
    for source, groups in ((left, left_groups), (right, right_groups)):
        # Avoid Python timezone conversion while comparing exact coordinates.
        for row in source.select(*keys, "start", "end").select(pl.all().to_physical()).iter_rows():
            groups.setdefault(row[:-2], []).append(row[-2:])
    rows = [
        (*key, start, end)
        for key, ranges in left_groups.items()
        for start, end in cells(
            ranges, right_groups.get(key, []), operation is pi.intersect_intervals
        )
    ]
    selected = left.select(*keys, "start", "end")
    physical_schema = selected.select(pl.all().to_physical()).schema
    return pl.DataFrame(rows, schema=physical_schema, orient="row").cast(selected.schema)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize(
    "left,right",
    [
        ([], []),
        ([(0, 4)], []),
        ([], [(0, 4)]),
        ([(2, 2)], [(0, 5)]),
        ([(0, 2)], [(2, 4)]),
        ([(0, 10)], [(0, 10)]),
        ([(0, 5), (4, 10), (12, 15)], [(2, 3), (6, 8), (10, 13)]),
        ([(0, 3), (5, 8)], [(1, 7)]),
        ([(0, 100), (1, 2), (3, 4)], [(50, 60)]),
        ([(0, 10)], [(2, 8), (3, 5), (8, 9), (2, 8)]),
        ([(0, 10)] * 20, [(0, 10)] * 15),
        ([(0, 2), (2, 4)], [(1, 3)]),
        ([(0, 10), (5, 5)], [(1, 1), (4, 4), (9, 9)]),
        ([(0, 100)], [(i, i + 1) for i in range(1, 100, 2)]),
    ],
)
def test_named_set_geometry(operation, left, right):
    expected = frame(cells(left, right, operation is pi.intersect_intervals))
    assert_frame_equal(operation(frame(left), frame(right)), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("empty", [False, True])
def test_exact_endpoint_schema_including_empty_results(operation, dtype, empty):
    left = frame([(0, 10), (12, 15)]).cast(dtype)
    right = frame([(2, 4), (8, 13)]).cast(dtype)
    if empty:
        right = left if operation is pi.subtract_intervals else left.clear()
    expected = frame(
        []
        if empty
        else (
            [(0, 2), (4, 8), (13, 15)]
            if operation is pi.subtract_intervals
            else [(2, 4), (8, 10), (12, 13)]
        )
    ).cast(dtype)
    assert_frame_equal(operation(left, right), expected)
    assert_frame_equal(operation(left.lazy(), right.lazy()).collect(), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize(
    "dtype,base",
    [(pl.Int64, -(2**63)), (pl.Int64, 2**63 - 10), (pl.UInt64, 2**64 - 10), (pl.UInt64, 2**53 + 1)],
)
def test_extreme_exact_coordinates(operation, dtype, base):
    left, right = [(base, base + 8)], [(base + 1, base + 2), (base + 4, base + 6)]
    assert_frame_equal(
        operation(frame(left, dtype), frame(right, dtype)),
        frame(cells(left, right, operation is pi.intersect_intervals), dtype),
    )


@pytest.mark.parametrize("operation", OPERATIONS)
def test_timezone_offset_transition_uses_exact_physical_timestamps(operation):
    # Helsinki repeats local 03:00 at the autumn offset change.
    dtype = pl.Datetime("ns", "Europe/Helsinki")
    base = 1792886400000000000
    left = frame([(base, base + 7200000000000)]).cast(dtype)
    right = frame([(base + 1, base + 3600000000001)]).cast(dtype)
    expected = frame(
        cells(
            left.cast(pl.Int64).rows(),
            right.cast(pl.Int64).rows(),
            operation is pi.intersect_intervals,
        )
    ).cast(dtype)
    assert_frame_equal(operation(left.lazy(), right).collect(engine="streaming"), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
def test_nullable_multikey_matching_and_original_left_order(operation):
    left = frame([(0, 0), (1, 10), (0, 5), (4, 12), (8, 10), (0, 8)]).with_columns(
        pl.Series("g", ["early", "later", None, "early", "left-only", None]),
        pl.Series("h", [None, 1, None, None, 1, 2], dtype=pl.Int32),
    )
    right = frame([(2, 4), (1, 2), (0, 9), (7, 9), (10, 10)]).with_columns(
        pl.Series("g", [None, "right-only", "early", "later", "left-only"]),
        pl.Series("h", [None, 1, None, 1, 1], dtype=pl.Int32),
    )
    expected = grouped_oracle(left, right, ["g", "h"], operation)
    for rhs in (right, right.reverse()):
        assert_frame_equal(operation(left, rhs, by=["g", "h"]), expected)
        assert_frame_equal(operation(left.lazy(), rhs.lazy(), by=["g", "h"]).collect(), expected)


def test_grouped_intersection_is_commutative_with_left_oriented_presentation():
    left = frame([(0, 5), (0, 8)]).with_columns(pl.Series("key", ["b", "a"]))
    right = frame([(1, 4), (2, 7)]).with_columns(pl.Series("key", ["a", "b"]))
    forward = pi.intersect_intervals(left, right, by="key")
    backward = pi.intersect_intervals(right, left, by="key")
    assert forward["key"].to_list() == ["b", "a"]
    assert backward["key"].to_list() == ["a", "b"]
    assert_frame_equal(forward.sort("key"), backward.sort("key"))


@pytest.mark.parametrize("operation", OPERATIONS)
def test_endpoint_columns_can_repeat_or_also_be_group_keys(operation):
    left = frame([(0, 10), (2, 8)]).rename({"start": "coordinate"})
    right = frame([(0, 4), (2, 6)]).rename({"start": "coordinate"})
    expected = operation(
        left.rename({"coordinate": "start"}).with_columns(pl.col("start").alias("coordinate")),
        right.rename({"coordinate": "start"}).with_columns(pl.col("start").alias("coordinate")),
        by="coordinate",
    )
    assert_frame_equal(
        operation(
            left.lazy(),
            right.lazy(),
            left_start="coordinate",
            right_start="coordinate",
            by="coordinate",
        ).collect(),
        expected,
    )
    assert (
        operation(
            left.lazy(),
            right.lazy(),
            left_start="coordinate",
            left_end="coordinate",
            right_start="coordinate",
            right_end="coordinate",
        )
        .collect()
        .is_empty()
    )


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize(
    "dtype",
    [pl.String, pl.Boolean, *INTEGER_DTYPES, pl.Date, pl.Datetime("ns", "Europe/Helsinki")],
    ids=str,
)
def test_supported_key_types_and_typed_empty_groups(operation, dtype):
    values = ["b", None, "a", "b"] if dtype == pl.String else [1, None, 0, 1]
    keys = pl.Series("key", values).cast(dtype)
    left = frame([(0, 0), (1, 8), (2, 9), (3, 10)]).with_columns(keys)
    right = frame([(4, 6), (5, 7), (3, 4), (10, 10)]).with_columns(keys.reverse())
    expected = grouped_oracle(left, right, ["key"], operation)
    assert_frame_equal(operation(left, right, by="key"), expected)
    assert_frame_equal(operation(left.clear().lazy(), right, by="key").collect(), expected.clear())


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("seed", range(12))
def test_random_original_row_oracle_and_group_isolation(operation, seed):
    rng = random.Random(seed)
    sources = []
    for _ in range(2):
        count = rng.randrange(30)
        rows = [
            tuple(sorted((rng.randrange(-12, 13), rng.randrange(-12, 13)))) for _ in range(count)
        ]
        sources.append(
            frame(rows).with_columns(
                pl.Series("key", [rng.choice([None, "a", "b", "c"]) for _ in rows], dtype=pl.String)
            )
        )
    left, right = sources
    for keys in ([], ["key"]):
        expected = grouped_oracle(left, right, keys, operation)
        assert_frame_equal(operation(left, right, by=keys), expected)
        assert_frame_equal(operation(left.lazy(), right.lazy(), by=keys).collect(), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("side", ["left", "right"])
@pytest.mark.parametrize("kind", ["reversed", "null"])
@pytest.mark.parametrize("lazy", [False, True])
def test_bad_original_rows_survive_empty_and_unmatched_shortcuts(operation, side, kind, lazy):
    valid = frame([(0, 1)]).with_columns(pl.lit("valid").alias("key"))
    bad = frame([(0, 10), (5, 5), (4, 2 if kind == "reversed" else None)]).with_columns(
        pl.Series("key", ["valid", "valid", "unmatched"])
    )
    for good in (valid, valid.clear()):
        left, right = (bad, good) if side == "left" else (good, bad)
        with pytest.raises(pl.exceptions.ComputeError, match=rf"{side}.*index 2"):
            if lazy:
                operation(left.lazy(), right.lazy(), by="key").head(1).collect()
            else:
                operation(left, right, by="key")


@pytest.mark.parametrize("operation", OPERATIONS)
def test_left_row_errors_take_precedence(operation):
    left = frame([(0, 1), (4, 3)])
    right = frame([(None, 1)])
    with pytest.raises(pl.exceptions.ComputeError, match="left.*index 1"):
        operation(left.lazy(), right.lazy()).collect()


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize(
    "left_dtype,right_dtype",
    [
        (pl.Int64, pl.UInt64),
        (pl.Int32, pl.Int64),
        (pl.Date, pl.Int32),
        (pl.Datetime("ms"), pl.Datetime("ns")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us", "Europe/Helsinki")),
    ],
)
def test_cross_side_schema_mismatch_even_with_empty_input(operation, left_dtype, right_dtype):
    for empty in (False, True):
        left = frame([] if empty else [(0, 1)]).cast(left_dtype)
        right = frame([(0, 1)]).cast(right_dtype)
        with pytest.raises(pl.exceptions.PolarsError, match="matching"):
            operation(left.lazy(), right.lazy())


@pytest.mark.parametrize("operation", OPERATIONS)
def test_invalid_options_keys_and_literal_column_names(operation):
    left = frame([(0, 10)]).with_columns(pl.lit(1).alias("key"))
    right = frame([(2, 4)]).with_columns(pl.lit(1).alias("key"))
    for bad in (None, [], {}, pl.Series([1])):
        for lhs, rhs in ((bad, right), (left, bad)):
            with pytest.raises(TypeError, match="DataFrame or LazyFrame"):
                operation(lhs, rhs)
    for bad in (True, ("key",), [1], pl.col("key")):
        with pytest.raises(TypeError, match="by"):
            operation(left, right, by=bad)
    for by in (["key", "key"], ["start"], ["end"]):
        with pytest.raises(ValueError):
            operation(left, right, by=by)
    for by in ("missing",):
        with pytest.raises(pl.exceptions.ColumnNotFoundError):
            operation(left, right, by=by)
    with pytest.raises(pl.exceptions.PolarsError, match="matching.*key"):
        operation(left, right.with_columns(pl.col("key").cast(pl.Int64)), by="key")
    for dtype in (pl.Float64, pl.List(pl.Int64), pl.Object, pl.Categorical):
        bad = right.with_columns(pl.Series("key", [None], dtype=dtype))
        with pytest.raises(pl.exceptions.PolarsError, match="group keys"):
            operation(left, bad, by="key")
    for argument in ("left_start", "left_end", "right_start", "right_end"):
        with pytest.raises(TypeError, match="strings"):
            operation(left, right, **{argument: pl.col("start")})
        with pytest.raises(pl.exceptions.ColumnNotFoundError):
            operation(left, right, **{argument: "missing"})

    lhs = left.rename({"start": "*", "end": "^end$", "key": "__pi_left"}).with_columns(
        pl.lit("payload").alias("__pi_row"), pl.lit([1, 2]).alias("irrelevant")
    )
    rhs = right.rename({"start": "^start$", "end": "*", "key": "__pi_left"})
    expected = operation(left, right, by="key").rename({"key": "__pi_left"})
    result = operation(
        lhs.lazy(),
        rhs.lazy(),
        left_start="*",
        left_end="^end$",
        right_start="^start$",
        right_end="*",
        by="__pi_left",
    ).collect()
    assert_frame_equal(result, expected)
