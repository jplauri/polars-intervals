"""Independent original-row cell oracle for the native Polars competitor."""

import random
from itertools import pairwise

import polars as pl
import pytest
from polars.testing import assert_frame_equal
from polars_intervals import _COVERAGE_STATS_SCHEMA

from benchmarks.coverage_stats_native import native_coverage_stats


def assert_complete(actual, expected):
    assert_frame_equal(actual, expected, rel_tol=1e-15, abs_tol=1e-15)


def oracle(queries, intervals, by=None):
    """Scan original source membership in each clipped elementary query cell."""
    keys = [] if by is None else [by] if isinstance(by, str) else by
    q = queries.with_columns(pl.col("start", "end").to_physical()).to_dicts()
    s = intervals.with_columns(pl.col("start", "end").to_physical()).to_dicts()
    stats = []
    for query in q:
        a, b = query["start"], query["end"]
        sources = [row for row in s if all(row[key] == query[key] for key in keys)]
        hits = [
            row
            for row in sources
            if row["start"] < row["end"] and row["start"] < b and row["end"] > a
        ]
        boundaries = sorted(
            {a, b, *(max(a, min(b, row[name])) for row in sources for name in ("start", "end"))}
        )
        covered = sum(
            end - start
            for start, end in pairwise(boundaries)
            if any(row["start"] <= start < row["end"] for row in sources)
        )
        length = b - a
        stats.append(
            (
                len(hits) if length else 0,
                covered,
                length,
                float(covered) / float(length) if length else None,
            )
        )
    return queries.hstack(pl.DataFrame(stats, schema=_COVERAGE_STATS_SCHEMA, orient="row"))


def test_native_random_original_cells_and_payloads():
    rng = random.Random(713)
    for grouped in (False, True):
        for _ in range(24):
            frames = []
            for _ in range(2):
                rows = []
                for _ in range(rng.randrange(18)):
                    start, end = sorted((rng.randrange(-8, 9), rng.randrange(-8, 9)))
                    rows.append((rng.choice([None, 0, 1, 3]), start, end))
                frame = pl.DataFrame(
                    rows,
                    schema={"group": pl.Int64, "start": pl.Int64, "end": pl.Int64},
                    orient="row",
                )
                frames.append(frame)
            queries, intervals = frames
            queries = queries.with_columns(
                pl.lit([1, None, 3]).alias("list"),
                pl.struct("group").alias("struct"),
                pl.lit("label").cast(pl.Categorical).alias("category"),
            )
            by = "group" if grouped else None
            expected = oracle(queries, intervals, by)
            assert_complete(native_coverage_stats(queries, intervals, by=by), expected)
            assert_complete(
                native_coverage_stats(queries.lazy(), intervals.lazy(), by=by).collect(
                    engine="streaming"
                ),
                expected,
            )


@pytest.mark.parametrize(
    "dtype,lo,hi",
    [
        (pl.Int64, -(2**63), 2**63 - 1),
        (pl.UInt64, 0, 2**64 - 1),
        (pl.Date, -10000, 10000),
        (pl.Datetime("ns", "UTC"), -(2**63), 2**63 - 1),
    ],
)
def test_native_exact_extreme_lengths(dtype, lo, hi):
    q = pl.DataFrame(
        {"start": [lo, lo + 1, hi], "end": [hi, hi - 1, hi]}, schema={"start": dtype, "end": dtype}
    )
    s = pl.DataFrame(
        {"start": [lo, lo + 1], "end": [hi - 1, hi]}, schema={"start": dtype, "end": dtype}
    )
    assert_complete(native_coverage_stats(q, s), oracle(q, s))


@pytest.mark.parametrize(
    "dtype,lo,hi", [(pl.Int64, -(2**63), 2**63 - 1), (pl.UInt64, 0, 2**64 - 1)]
)
def test_native_random_wide_integer_fractions(dtype, lo, hi):
    rng = random.Random(841)
    for _ in range(24):
        frames = []
        for count in (7, 11):
            rows = [sorted((rng.randint(lo, hi), rng.randint(lo, hi))) for _ in range(count)]
            frames.append(pl.DataFrame(rows, schema={"start": dtype, "end": dtype}, orient="row"))
        queries, intervals = frames
        assert_complete(native_coverage_stats(queries, intervals), oracle(queries, intervals))


@pytest.mark.parametrize("bad", [None, 9])
def test_native_invalid_unmatched_source_and_empty_queries(bad):
    q = pl.DataFrame({"group": [0], "start": [0], "end": [1]})
    s = pl.DataFrame({"group": [1, 2], "start": [0, bad], "end": [1, 2]})
    for queries in (q, q.head(0)):
        plan = native_coverage_stats(queries.lazy(), s.lazy(), by="group")
        with pytest.raises(pl.exceptions.ComputeError, match="intervals.*1"):
            plan.select(pl.len()).collect()


def test_native_deferred_complete_sources_and_downstream_projection():
    calls = []
    q = pl.DataFrame({"start": [0, 5], "end": [10, 10], "id": [0, 0]})
    s = pl.DataFrame({"start": [6, 0], "end": [8, 4]})

    def record(frame):
        calls.append(frame.height)
        return frame

    plan = native_coverage_stats(
        q.lazy().map_batches(record, schema=q.schema), s.lazy().map_batches(record, schema=s.schema)
    )
    plan.explain()
    plan.collect_schema()
    assert not calls
    assert_complete(plan.head(1).collect(), oracle(q, s).head(1))
    assert_complete(plan.select("covered_length").collect(), oracle(q, s).select("covered_length"))


def test_native_benchmark_fixtures_match_cells():
    from benchmarks.coverage_stats import CASES, fixture

    for case in CASES:
        for size in (0, 8, 32):
            q, s, options = fixture(size, case, 7)
            assert_complete(native_coverage_stats(q, s, **options), oracle(q, s, options["by"]))


def test_native_null_composite_keys_and_custom_literal_names():
    q = pl.DataFrame(
        {
            "start": [None, "a", None],
            "key": [True, None, True],
            "*": [0, 5, 0],
            "^end$": [10, 10, 10],
        }
    )
    s = pl.DataFrame({"start": [None, "a"], "key": [True, None], "*": [1, 6], "^end$": [7, 8]})
    out = native_coverage_stats(
        q,
        s,
        query_start="*",
        query_end="^end$",
        interval_start="*",
        interval_end="^end$",
        by=["start", "key"],
    )
    assert out["overlap_count"].to_list() == [1, 1, 1]
    assert out["covered_length"].to_list() == [6, 2, 6]
    assert_frame_equal(out.select(pl.selectors.by_name(*q.columns)), q)
