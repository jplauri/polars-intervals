"""Original-row Boolean cell oracle for the measured Polars competitor."""

import random
from itertools import pairwise

import polars as pl
import pytest
from polars.testing import assert_frame_equal

from benchmarks.set_geometry_native import native_set_geometry


def oracle(left, right, intersection, by=None):
    keys = [] if by is None else [by]
    schema = left.select(*keys, "start", "end").schema
    left, right = [
        frame.with_columns(pl.col("start", "end").to_physical()) for frame in (left, right)
    ]
    output = []
    groups = list(dict.fromkeys(tuple(row[key] for key in keys) for row in left.to_dicts()))
    for key in groups:
        a, b = [
            [row for row in frame.to_dicts() if tuple(row[name] for name in keys) == key]
            for frame in (left, right)
        ]
        endpoints = sorted({row[name] for row in a + b for name in ("start", "end")})
        runs = []
        for start, end in pairwise(endpoints):
            in_left = any(row["start"] <= start < row["end"] for row in a)
            in_right = any(row["start"] <= start < row["end"] for row in b)
            if in_left and (in_right if intersection else not in_right):
                if runs and runs[-1][1] == start:
                    runs[-1] = (runs[-1][0], end)
                else:
                    runs.append((start, end))
        output.extend((*key, start, end) for start, end in runs)
    return pl.DataFrame(
        output, schema=left.select(*keys, "start", "end").schema, orient="row"
    ).cast(schema)


@pytest.mark.parametrize("intersection", [False, True])
def test_native_matches_original_cells(intersection):
    rng = random.Random(713)
    for grouped in (False, True):
        for _ in range(32):
            frames = []
            for _ in range(2):
                rows = []
                for _ in range(rng.randrange(20)):
                    start, end = sorted((rng.randrange(-8, 9), rng.randrange(-8, 9)))
                    rows.append((rng.choice([None, 0, 1, 3]), start, end))
                frame = pl.DataFrame(
                    rows,
                    schema={"group": pl.Int64, "start": pl.Int64, "end": pl.Int64},
                    orient="row",
                )
                frames.append(frame if grouped else frame.drop("group"))
            left, right = frames
            by = "group" if grouped else None
            expected = oracle(left, right, intersection, by)
            assert_frame_equal(
                native_set_geometry(left, right, intersection=intersection, by=by), expected
            )
            assert_frame_equal(
                native_set_geometry(
                    left.lazy(), right.lazy(), intersection=intersection, by=by
                ).collect(engine="streaming"),
                expected,
            )


@pytest.mark.parametrize("intersection", [False, True])
@pytest.mark.parametrize("invalid_start", [8, None])
def test_validation_survives_empty_left_and_unmatched_group(intersection, invalid_start):
    left = pl.DataFrame({"group": [0], "start": [0], "end": [0]})
    right = pl.DataFrame({"group": [1, 2], "start": [2, invalid_start], "end": [4, 7]})
    for lhs in (left, left.head(0)):
        plan = native_set_geometry(lhs.lazy(), right.lazy(), intersection=intersection, by="group")
        with pytest.raises(pl.exceptions.ComputeError, match="right interval at index 1"):
            plan.select(pl.len()).collect()


def test_native_deferred_and_exact_metadata():
    calls = []
    frame = pl.DataFrame(
        {"start": [2**63 + 1], "end": [2**63 + 5]}, schema={"start": pl.UInt64, "end": pl.UInt64}
    )

    def record(value):
        calls.append(value.height)
        return value

    lazy = frame.lazy().map_batches(record, schema=frame.schema, streamable=False)
    result = native_set_geometry(lazy, lazy, intersection=True)
    result.explain()
    assert result.collect_schema() == frame.schema
    assert not calls
    assert_frame_equal(result.collect(), frame)
    assert calls


def test_native_benchmark_fixtures_match_cells():
    from benchmarks.set_geometry import CASES, fixture

    for case in CASES:
        for n in (0, 8, 32):
            left, right, options = fixture(n, case, 7)
            for intersection in (False, True):
                assert_frame_equal(
                    native_set_geometry(left, right, intersection=intersection, **options),
                    oracle(left, right, intersection, options["by"]),
                )


def test_native_null_multikey_order_and_coalescing():
    left = pl.DataFrame(
        {
            "a": [None, "second", None],
            "b": [True, None, True],
            "start": [0, 0, 0],
            "end": [0, 10, 10],
        }
    )
    right = pl.DataFrame(
        {"a": ["second", None, None], "b": [None, True, True], "start": [2, 2, 4], "end": [6, 4, 6]}
    )
    expected = pl.DataFrame(
        {"a": [None, "second"], "b": [True, None], "start": [2, 2], "end": [6, 6]}
    )
    assert_frame_equal(native_set_geometry(left, right, intersection=True, by=["a", "b"]), expected)
    assert_frame_equal(
        native_set_geometry(left.lazy(), right.lazy(), intersection=True, by=["a", "b"])
        .select("end")
        .head(1)
        .collect(),
        expected.select("end").head(1),
    )


def test_native_duplicate_depth_and_real_gaps():
    left = pl.DataFrame({"start": [0, 0], "end": [10, 10]})
    right = pl.DataFrame({"start": [0, 3, 6], "end": [2, 5, 10]})
    expected = pl.DataFrame({"start": [2, 5], "end": [3, 6]})
    assert_frame_equal(native_set_geometry(left, right), expected)
    assert_frame_equal(
        native_set_geometry(left.lazy(), right.lazy()).filter(pl.col("start") >= 5).collect(),
        expected.tail(1),
    )
