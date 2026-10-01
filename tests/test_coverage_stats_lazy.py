"""Both operand plans, payload identity, optimizer barriers and real scans."""

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .test_coverage_stats import assert_stats, frame, oracle


@pytest.mark.parametrize(
    "qlazy,slazy", [(False, False), (True, False), (False, True), (True, True)]
)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_all_frame_kinds_defer_both_inputs(qlazy, slazy, engine):
    q, s = frame([(0, 10), (5, 10), (7, 7)]), frame([(1, 7), (4, 9)])
    seen = [[], []]

    def instrument(df, side, lazy):
        def record(batch):
            seen[side].append(batch.height)
            return batch

        return df.lazy().map_batches(record, schema=df.schema, streamable=False) if lazy else df

    result = pi.coverage_stats(instrument(q, 0, qlazy), instrument(s, 1, slazy))
    expected = oracle(q, s)
    if qlazy or slazy:
        assert isinstance(result, pl.LazyFrame)
        assert result.collect_schema() == expected.schema
        assert result.explain()
        assert seen == [[], []]
        for _ in range(2):
            assert_stats(result.collect(engine=engine), expected)
        assert bool(seen[0]) == qlazy
        assert bool(seen[1]) == slazy
    else:
        assert isinstance(result, pl.DataFrame)
        assert_stats(result, expected)
    assert_stats(q.lazy().pipe(pi.coverage_stats, s.lazy()).collect(engine=engine), expected)


TRANSFORMS = {
    "query_start": lambda df: df.filter(pl.col("start") >= 5),
    "fraction": lambda df: df.filter(pl.col("covered_fraction") < 0.9),
    "count_filter": lambda df: df.filter(pl.col("overlap_count") > 1),
    "length_only": lambda df: df.select("covered_length"),
    "payload_only": lambda df: df.select("id", "key"),
    "row_count": lambda df: df.select(pl.len()),
    "head": lambda df: df.head(1),
    "tail": lambda df: df.tail(1),
    "slice": lambda df: df.slice(1, 2),
}


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
def test_downstream_transforms_retain_complete_sources(engine, transform):
    q = frame([(0, 10), (5, 10), (7, 7), (20, 21), (0, 10)]).with_columns(
        pl.Series("key", ["a", "a", None, "missing", "a"]), pl.lit(1).alias("id")
    )
    s = frame([(6, 8), (0, 4), (0, 8), (0, 4)]).with_columns(
        pl.Series("key", ["a", "a", None, "a"])
    )
    expected = transform(oracle(q, s, ["key"]))
    result = transform(pi.coverage_stats(q.lazy(), s.lazy(), by="key"))
    assert_frame_equal(result.collect(engine=engine), expected)
    assert_frame_equal(
        result.collect(engine=engine, optimizations=pl.QueryOptFlags.none()), expected
    )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_slice_and_query_predicate_do_not_leak_into_sources(engine):
    q, s = frame([(0, 10)]), frame([(6, 8), (0, 4)])
    out = pi.coverage_stats(q.lazy(), s.lazy()).head(1).collect(engine=engine)
    assert out.select("overlap_count", "covered_length").row(0) == (2, 6)
    q, s = frame([(0, 10), (5, 10)]), frame([(0, 8)])
    out = pi.coverage_stats(q.lazy(), s.lazy()).filter(pl.col("start") >= 5).collect(engine=engine)
    assert out["covered_length"].item() == 3
    # Filtering queries with sources fixed is numerically valid.
    before = pi.coverage_stats(q.lazy().filter(pl.col("start") >= 5), s.lazy()).collect(
        engine=engine
    )
    assert_frame_equal(before, out)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_arbitrary_chunks_and_both_parquet_scans(engine, tmp_path):
    q = frame([(0, 10), (3, 3), (2, 8), (0, 10), (20, 25), (1, 9)]).with_columns(
        pl.Series("key", ["a", None, "b", "a", "missing", "b"]),
        pl.Series("payload", [[1], None, [], [4], [5], [6]]),
    )
    s = frame([(100, 101), (3, 7), (0, 4), (6, 8), (0, 4)]).with_columns(
        pl.Series("key", ["source-only", "b", "a", "a", "a"])
    )
    expected = oracle(q, s, ["key"])
    chunks, scans = [], []
    for name, df, lengths in (("queries", q, [0, 2, 1, 3]), ("intervals", s, [1, 0, 2, 2])):
        parts, paths, offset = [], [], 0
        for i, length in enumerate(lengths):
            part = df.slice(offset, length)
            offset += length
            parts.append(part)
            path = tmp_path / f"{name}-{i}.parquet"
            part.write_parquet(path, row_group_size=1)
            paths.append(path)
        chunks.append(pl.concat(parts, rechunk=False))
        scans.append(pl.scan_parquet(paths))
    assert all(df.n_chunks() > 1 for df in chunks)
    assert_stats(pi.coverage_stats(*chunks, by="key"), expected)
    assert_stats(pi.coverage_stats(*(df.rechunk() for df in chunks), by="key"), expected)
    assert_stats(
        pi.coverage_stats(*(df.lazy() for df in chunks), by="key").collect(engine=engine), expected
    )
    assert_stats(pi.coverage_stats(*scans, by="key").collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_shared_lazy_source_and_independently_filtered_branches(engine):
    df = frame([(0, 10), (2, 4), (6, 8), (3, 3)])
    lazy = df.lazy()
    assert_stats(pi.coverage_stats(lazy, lazy).collect(engine=engine), oracle(df, df))
    q, s = lazy.filter(pl.col("start") == 0), lazy.filter(pl.col("start") > 0)
    assert_stats(pi.coverage_stats(q, s).collect(engine=engine), oracle(df.head(1), df.tail(3)))


def test_literal_names_custom_endpoint_keys_and_frozen_options():
    q = (
        frame([(0, 10), (5, 10)])
        .rename({"start": "*", "end": "^end$"})
        .with_columns(
            pl.Series("start", ["a", "b"]),
            pl.Series("end", [1, 2]),
            pl.lit(7).alias("__pi_coverage_start"),
            pl.lit(8).alias("__pi_coverage__query"),
        )
    )
    s = (
        frame([(1, 9), (2, 7)])
        .rename({"start": "^s$", "end": "*"})
        .with_columns(pl.Series("start", ["a", "b"]), pl.Series("end", [1, 2]))
    )
    keys = ["start", "end"]
    options = {
        "query_start": "*",
        "query_end": "^end$",
        "interval_start": "^s$",
        "interval_end": "*",
        "by": keys,
    }
    expected = pi.coverage_stats(q, s, **options)
    result = pi.coverage_stats(q.lazy(), s.lazy(), **options)
    keys[:] = ["does not exist"]
    assert_frame_equal(result.collect(), expected)
    assert expected["covered_length"].to_list() == [8, 2]
    assert_frame_equal(expected.select(pl.selectors.by_name(q.columns)), q)


@pytest.mark.parametrize("side", ["queries", "intervals"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize(
    "transform",
    [
        lambda x: x.head(1),
        lambda x: x.select("covered_length"),
        lambda x: x.select("key"),
        lambda x: x.select(pl.len()),
    ],
)
def test_invalid_late_rows_keep_original_operand_indices(side, engine, transform, tmp_path):
    good = frame([(0, 10)]).with_columns(pl.lit("a").alias("key"))
    bad = frame([(1, 2), (100, 101), (1000, 999)]).with_columns(
        pl.Series("key", ["a", "source-only", "source-only"])
    )
    path = tmp_path / f"{side}.parquet"
    bad.write_parquet(path, row_group_size=1)
    scan = pl.scan_parquet(path)
    q, s = (scan, good) if side == "queries" else (good, scan)
    with pytest.raises(pl.exceptions.ComputeError, match=rf"{side}.*index 2"):
        transform(pi.coverage_stats(q, s, by="key")).collect(engine=engine)


@pytest.mark.parametrize("queries", [[], [(2, 2)], [(1000, 1001)]])
@pytest.mark.parametrize("lazy", [False, True])
def test_invalid_sources_validate_before_empty_and_unmatched_shortcuts(queries, lazy):
    q = frame(queries).with_columns(pl.lit("query-only").alias("key"))
    s = frame([(0, 1), (4, 3)]).with_columns(pl.lit("source-only").alias("key"))
    with pytest.raises(pl.exceptions.ComputeError, match="intervals.*index 1"):
        result = (
            pi.coverage_stats(q.lazy(), s.lazy(), by="key")
            if lazy
            else pi.coverage_stats(q, s, by="key")
        )
        if lazy:
            result.collect()


@pytest.mark.parametrize("side", ["queries", "intervals"])
def test_null_errors_identify_original_row(side):
    bad = pl.DataFrame({"start": [1, 2, None], "end": [2, 3, 4]})
    good = frame([(0, 10)])
    q, s = (bad, good) if side == "queries" else (good, bad)
    with pytest.raises(pl.exceptions.ComputeError, match=rf"{side}.*null endpoints.*index 2"):
        pi.coverage_stats(q.lazy(), s.lazy()).collect()


def test_incorrect_sorted_metadata_does_not_replace_verified_order():
    q = frame([(10, 20), (0, 100), (4, 9), (2, 10)])
    s = frame([(12, 15), (1, 7), (2, 2), (0, 10)])
    expected = oracle(q, s)
    assert_stats(
        pi.coverage_stats(q.lazy().set_sorted("start"), s.lazy().set_sorted("start")).collect(),
        expected,
    )
