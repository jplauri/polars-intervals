"""Original-row/cell oracles and the public coverage contract."""

import random
from datetime import UTC, datetime
from itertools import pairwise

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES

STATS = {
    "overlap_count": pl.UInt64,
    "covered_length": pl.Int128,
    "query_length": pl.Int128,
    "covered_fraction": pl.Float64,
}


def frame(rows, dtype=pl.Int64):
    return pl.DataFrame(rows, schema={"start": dtype, "end": dtype}, orient="row")


def oracle(queries, intervals, keys=(), qs="start", qe="end", ss="start", se="end"):
    """Inspect original source membership in every elementary clipped cell."""
    sources = intervals.with_columns(pl.selectors.by_name(ss, se, *keys).to_physical()).to_dicts()
    result = []
    for query in queries.with_columns(pl.selectors.by_name(qs, qe, *keys).to_physical()).to_dicts():
        a, b = query[qs], query[qe]
        matching = [row for row in sources if all(query[k] == row[k] for k in keys)]
        hits = [
            row for row in matching if a < b and row[ss] < row[se] and row[ss] < b and row[se] > a
        ]
        boundaries = sorted(
            {a, b, *(max(a, row[ss]) for row in hits), *(min(b, row[se]) for row in hits)}
        )
        covered = sum(
            y - x
            for x, y in pairwise(boundaries)
            if any(row[ss] <= x and y <= row[se] for row in matching)
        )
        result.append((len(hits), covered, b - a, float(covered) / float(b - a) if a < b else None))
    return queries.hstack(pl.DataFrame(result, schema=STATS, orient="row"))


def assert_stats(actual, expected):
    assert_frame_equal(actual, expected, check_exact=False, rel_tol=1e-15, abs_tol=1e-15)
    assert_frame_equal(
        actual.drop("covered_fraction"), expected.drop("covered_fraction"), check_exact=True
    )
    assert (
        actual["covered_fraction"].is_null().to_list()
        == expected["covered_fraction"].is_null().to_list()
    )


def test_example_and_duplicate_query_identity():
    queries = frame([(0, 10), (5, 10), (12, 15), (7, 7), (0, 10)]).with_columns(
        pl.Series("id", [3, 3, None, 0, 3])
    )
    sources = frame([(1, 7), (4, 9)])
    out = pi.coverage_stats(queries, sources)
    assert out.select(*STATS).rows() == [
        (2, 8, 10, 0.8),
        (2, 4, 5, 0.8),
        (0, 0, 3, 0.0),
        (0, 0, 0, None),
        (2, 8, 10, 0.8),
    ]
    assert_stats(out, oracle(queries, sources))


@pytest.mark.parametrize(
    "queries,sources",
    [
        ([], []),
        ([], [(0, 2)]),
        ([(0, 2), (1, 1)], []),
        ([(0, 0), (1, 1), (2, 2), (10, 10)], [(0, 2), (1, 1), (1, 1)]),
        ([(0, 4), (0, 2), (2, 4)], [(0, 2), (2, 4), (0, 0), (2, 2), (4, 4)]),
        ([(0, 10)], [(0, 2), (3, 5), (6, 8)]),
        ([(0, 10), (3, 4), (-1, 20)], [(1, 7), (1, 7), (2, 4), (-2, 15)]),
        ([(20, 22), (-3, -1), (0, 20), (3, 5), (8, 14)], [(1, 3), (5, 8), (10, 12), (15, 17)]),
        ([(0, 20), (1, 19), (2, 18), (3, 17), (3, 17)], [(20, 20), (15, 25), (2, 2), (1, 4)]),
    ],
)
@pytest.mark.parametrize("lazy", [False, True])
def test_named_edge_geometries(queries, sources, lazy):
    q, s = frame(queries), frame(sources)
    out = pi.coverage_stats(q.lazy(), s.lazy()).collect() if lazy else pi.coverage_stats(q, s)
    assert_stats(out, oracle(q, s))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("lazy", [False, True])
def test_supported_endpoints_preserve_physical_lengths(dtype, lazy):
    q, s = frame([(0, 10), (5, 10), (7, 7)], dtype), frame([(1, 7), (4, 9)], dtype)
    result = pi.coverage_stats(q.lazy(), s.lazy()).collect() if lazy else pi.coverage_stats(q, s)
    assert_stats(result, oracle(q, s))
    assert result.schema == {**q.schema, **STATS}


@pytest.mark.parametrize(
    "dtype,queries,sources",
    [
        (pl.Int8, [(-128, 127)], [(-128, 127)]),
        (
            pl.Int64,
            [(-(2**63), 2**63 - 1), (2**63 - 3, 2**63 - 2), (2**63 - 2, 2**63 - 1)],
            [(-(2**63), 2**63 - 2)],
        ),
        (pl.UInt64, [(0, 2**64 - 1), (2**64 - 2, 2**64 - 1)], [(0, 2**64 - 2)]),
        (pl.Int64, [(2**53 + 1, 2**53 + 2)], [(2**53, 2**53 + 3)]),
        (pl.Datetime("ns"), [(2**53 + 1, 2**53 + 2)], [(0, 2**53 + 3)]),
    ],
)
@pytest.mark.parametrize("lazy", [False, True])
def test_extremes_and_prefix_cancellation_use_python_big_integer_oracle(
    dtype, queries, sources, lazy
):
    q, s = frame(queries, dtype), frame(sources, dtype)
    result = pi.coverage_stats(q.lazy(), s.lazy()).collect() if lazy else pi.coverage_stats(q, s)
    assert_stats(result, oracle(q, s))


def test_timezone_transition_is_elapsed_physical_time():
    q = pl.DataFrame(
        {
            "start": [datetime(2026, 3, 28, 22, tzinfo=UTC)],
            "end": [datetime(2026, 3, 29, 21, tzinfo=UTC)],
        }
    ).with_columns(pl.all().dt.convert_time_zone("Europe/Helsinki"))
    assert q.select(pl.all().dt.hour()).row(0) == (0, 0)
    result = pi.coverage_stats(q.lazy(), q.lazy()).collect()
    assert result["query_length"].item() == 23 * 60 * 60 * 1_000_000
    assert result["covered_fraction"].item() == 1.0


@pytest.mark.parametrize("dtype", [pl.String, pl.Boolean, *ENDPOINT_DTYPES], ids=str)
def test_supported_nullable_keys(dtype):
    values = ["a", None, "b", "a"] if dtype == pl.String else [1, None, 0, 1]
    if dtype == pl.Boolean:
        values = [True, None, False, True]
    q = frame([(0, 10)] * 4).with_columns(pl.Series("key", values, dtype=dtype))
    s = q.head(2)
    assert_stats(pi.coverage_stats(q, s, by="key"), oracle(q, s, ["key"]))


def test_nullable_composite_keys_restore_global_order_and_ignore_source_payload():
    q = frame([(0, 10)] * 6).with_columns(
        pl.Series("chrom", ["a", None, "a", "missing", None, "a"]),
        pl.Series("key", [1, 1, None, 1, None, 1]),
    )
    s = frame([(1, 4), (5, 8), (0, 9), (1, 2), (100, 100)]).with_columns(
        pl.Series("chrom", ["a", None, "a", None, "source-only"]),
        pl.Series("key", [1, 1, None, None, 2]),
        pl.lit("ignored").alias("covered_length"),
    )
    for lazy in (False, True):
        out = (
            pi.coverage_stats(q.lazy(), s.lazy(), by=["chrom", "key"]).collect()
            if lazy
            else pi.coverage_stats(q, s, by=["chrom", "key"])
        )
        assert_stats(out, oracle(q, s, ["chrom", "key"]))


@pytest.mark.parametrize("lazy", [False, True])
def test_arbitrary_payloads_are_preserved_outside_native_import(lazy):
    q = (
        frame([(0, 10), (2, 8), (3, 3)])
        .with_columns(
            pl.Series("id", [1, 1, 1]),
            pl.Series("text", ["a", None, "a"]),
            pl.Series("list", [[1, None], [], None]),
            pl.Series("struct", [{"x": 1}, None, {"x": 3}]),
            pl.Series("cat", ["a", "b", None], dtype=pl.Categorical),
            pl.Series("enum", ["a", "b", None], dtype=pl.Enum(["a", "b"])),
            pl.Series("array", [[1, 2], None, [3, 4]], dtype=pl.Array(pl.Int64, 2)),
            pl.Series("decimal", [1, None, 3], dtype=pl.Decimal(20, 2)),
            pl.Series("binary", [b"a", b"b", None]),
            pl.Series("null", [None] * 3),
            pl.Series("wide", [2**100, None, 0], dtype=pl.Int128),
        )
        .select(pl.all().reverse())
    )
    s = frame([(0, 4), (6, 8)]).with_columns(
        pl.Series("unsupported-source", [object(), object()], dtype=pl.Object)
    )
    snapshots = q.clone(), s.clone()
    out = pi.coverage_stats(q.lazy(), s.lazy()).collect() if lazy else pi.coverage_stats(q, s)
    assert_frame_equal(out.select(q.columns), q)
    assert_stats(out, oracle(q, s))
    assert_frame_equal(q, snapshots[0])
    assert_frame_equal(s.drop("unsupported-source"), snapshots[1].drop("unsupported-source"))


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_object_payload_bypasses_native_series_import(engine):
    objects = [object(), None, object()]
    q = (
        frame([(0, 10), (3, 3), (1, 4)])
        .with_columns(pl.Series("object", objects, dtype=pl.Object))
        .select("object", "end", "start")
    )
    s = frame([(1, 7)])
    for result in (
        pi.coverage_stats(q, s),
        pi.coverage_stats(q.lazy(), s.lazy()).collect(engine=engine),
    ):
        assert result.columns[:3] == q.columns
        assert result["object"].to_list() == objects
        assert result["covered_length"].to_list() == [6, 0, 3]


@pytest.mark.parametrize("name", STATS)
@pytest.mark.parametrize("empty", [False, True])
def test_all_output_name_collisions_fail_before_execution(name, empty):
    q = frame([] if empty else [(0, 1)]).with_columns(pl.lit(0).alias(name))
    for queries in (q, q.lazy()):
        with pytest.raises(ValueError, match="queries.*output columns"):
            pi.coverage_stats(queries, frame([]))


@pytest.mark.parametrize(
    "kwargs",
    [
        {"by": ["g", "g"]},
        {"by": ("g",)},
        {"by": [1]},
        {"by": 1},
        {"query_start": 1},
        {"query_end": pl.col("end")},
        {"interval_start": None},
        {"interval_end": ["end"]},
    ],
)
def test_invalid_options(kwargs):
    with pytest.raises((TypeError, ValueError)):
        pi.coverage_stats(frame([]), frame([]), **kwargs)


@pytest.mark.parametrize("side", [0, 1])
def test_invalid_frames_and_missing_literal_columns(side):
    inputs = [frame([]), frame([])]
    inputs[side] = []
    with pytest.raises(TypeError, match="queries" if side == 0 else "intervals"):
        pi.coverage_stats(*inputs)
    with pytest.raises(pl.exceptions.ColumnNotFoundError):
        pi.coverage_stats(frame([]), frame([]), query_start="missing")


@pytest.mark.parametrize(
    "dtype", [pl.Float64, pl.Null, pl.Categorical, pl.List(pl.Int64), pl.Struct({"x": pl.Int64})]
)
def test_unsupported_keys_fail_without_native_import(dtype):
    q = frame([]).with_columns(pl.Series("key", [], dtype=dtype))
    with pytest.raises(pl.exceptions.InvalidOperationError, match="group keys"):
        pi.coverage_stats(q.lazy(), q.lazy(), by="key")


@pytest.mark.parametrize("lazy", [False, True])
def test_mismatched_empty_types_and_key_metadata(lazy):
    for dtype in [pl.UInt64, pl.Date, pl.Datetime("ns")]:
        q, s = frame([]), frame([], dtype)
        with pytest.raises(pl.exceptions.InvalidOperationError, match="matching"):
            pi.coverage_stats(q.lazy() if lazy else q, s)
    q = frame([]).with_columns(pl.Series("key", [], dtype=pl.Datetime("ns", "UTC")))
    s = frame([]).with_columns(pl.Series("key", [], dtype=pl.Datetime("ns", "Europe/Helsinki")))
    with pytest.raises(pl.exceptions.InvalidOperationError, match="group key dtypes"):
        pi.coverage_stats(q.lazy() if lazy else q, s, by="key")


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_random_independent_cell_and_bitmap_oracles(engine):
    rng = random.Random(412)
    for _ in range(50):
        q, s = [
            frame(
                [
                    tuple(sorted((rng.randrange(-8, 13), rng.randrange(-8, 13))))
                    for _ in range(rng.randrange(16))
                ]
            )
            for _ in range(2)
        ]
        expected = oracle(q, s)
        for a, b, covered in expected.select("start", "end", "covered_length").iter_rows():
            assert covered == sum(any(c <= x < d for c, d in s.iter_rows()) for x in range(a, b))
        assert_stats(pi.coverage_stats(q, s), expected)
        assert_stats(pi.coverage_stats(q.lazy(), s.lazy()).collect(engine=engine), expected)
        # Duplicating sources affects multiplicity only. Query duplication and
        # source permutation do not change any other reporting window.
        doubled = pi.coverage_stats(q, pl.concat([s, s]))
        assert doubled["overlap_count"].to_list() == (expected["overlap_count"] * 2).to_list()
        assert_frame_equal(doubled.drop("overlap_count"), expected.drop("overlap_count"))
        assert_stats(pi.coverage_stats(q.reverse(), s.reverse()), expected.reverse())
        assert_stats(pi.coverage_stats(pl.concat([q, q]), s), pl.concat([expected, expected]))


def test_self_inclusion_and_nonuniform_relabeling():
    q = frame([(0, 10), (2, 8), (2, 8), (3, 3)])
    out = pi.coverage_stats(q, q)
    counts = q.select(pi.overlap_count("start", "end")).to_series().to_list()
    assert out["overlap_count"].to_list() == [
        count + int(a < b) for (a, b), count in zip(q.rows(), counts)
    ]
    # Order alone preserves intersections but does not preserve distance.
    a = pi.coverage_stats(frame([(0, 4)]), frame([(0, 2)]))
    b = pi.coverage_stats(frame([(0, 10)]), frame([(0, 2)]))
    assert a["overlap_count"].item() == b["overlap_count"].item() == 1
    assert a["covered_fraction"].item() == 0.5
    assert b["covered_fraction"].item() == 0.2


def test_empty_key_list_and_same_endpoint_column_are_supported():
    q, s = frame([(0, 10), (1, 9)]), frame([(0, 10)])
    assert_frame_equal(pi.coverage_stats(q, s, by=[]), pi.coverage_stats(q, s))
    out = pi.coverage_stats(q.lazy(), s.lazy(), query_end="start").collect()
    assert out["overlap_count"].to_list() == [0, 0]
    assert out["query_length"].to_list() == [0, 0]
    assert out["covered_fraction"].to_list() == [None, None]


@pytest.mark.parametrize(
    "query_type,source_type",
    [
        (pl.Int64, pl.UInt64),
        (pl.Date, pl.Int32),
        (pl.Datetime("ns"), pl.Datetime("us")),
        (pl.Datetime("ms", "UTC"), pl.Datetime("ms")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us", "Europe/Helsinki")),
    ],
)
@pytest.mark.parametrize("empty", [False, True])
def test_cross_operand_metadata_is_exact(query_type, source_type, empty):
    q, s = [frame([] if empty else [(0, 1)], dtype) for dtype in (query_type, source_type)]
    with pytest.raises(pl.exceptions.InvalidOperationError, match="queries/intervals.*matching"):
        pi.coverage_stats(q.lazy(), s.lazy())
