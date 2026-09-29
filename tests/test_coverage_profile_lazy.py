"""Deferred whole-collection profiles must preserve the native exact contract."""

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES
from .test_coverage_profile import frame, oracle


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_construction_and_schema_do_not_execute_input(engine):
    source = frame([(0, 4, 2), (2, 5, 3), (5, 7, 3)])
    observed = []

    def record_execution(batch):
        observed.append(batch.height)
        return batch

    deferred = source.lazy().map_batches(record_execution, schema=source.schema, streamable=False)
    query = pi.coverage_profile(deferred, weight="q")
    assert isinstance(query, pl.LazyFrame)
    assert not observed
    assert query.collect_schema() == {"start": pl.Int64, "end": pl.Int64, "load": pl.Int128}
    assert query.select("load").collect_schema() == {"load": pl.Int128}
    assert not observed
    actual = query.collect(engine=engine)
    assert observed == [source.height]
    assert actual.rows() == oracle(source.rows(), include_zero=False)
    assert_frame_equal(actual, pi.coverage_profile(source, weight="q"))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_lazy_schema_preserves_endpoint_and_key_metadata_even_when_empty(dtype, engine):
    key_dtype = pl.Datetime("ns", "Europe/Helsinki")
    source = pl.DataFrame(
        {
            "start": pl.Series([0, 2], dtype=dtype),
            "end": pl.Series([2, 4], dtype=dtype),
            "q": pl.Series([0, 0], dtype=pl.UInt64),
            "resource": pl.Series([1, None], dtype=key_dtype),
        }
    )
    for data in (source, source.clear()):
        query = pi.coverage_profile(data.lazy(), weight="q", by="resource", include_zero=True)
        expected = pi.coverage_profile(data, weight="q", by="resource", include_zero=True)
        assert query.collect_schema() == {
            "resource": key_dtype,
            "start": dtype,
            "end": dtype,
            "load": pl.Int128,
        }
        assert_frame_equal(query.collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("weight", [None, "q"])
def test_empty_lazy_inputs_and_observed_empty_groups_with_explicit_domain(engine, weight):
    source = frame([(1, 1, 0), (2, 2, 4)]).with_columns(
        pl.Series("resource", [None, "a"], dtype=pl.String)
    )
    for data in (source.clear(), source):
        for by in (None, "resource"):
            for include_zero in (False, True):
                options = {
                    "weight": weight,
                    "by": by,
                    "domain_start": -1,
                    "domain_end": 5,
                    "include_zero": include_zero,
                }
                expected = pi.coverage_profile(data, **options)
                actual = pi.coverage_profile(data.lazy(), **options).collect(engine=engine)
                assert_frame_equal(actual, expected)
                if by is None and include_zero:
                    assert actual.rows() == [(-1, 5, 0)]
                elif data.is_empty():
                    assert actual.is_empty()


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("weight", [None, "q"])
@pytest.mark.parametrize("domain", [None, (-3, 8), (2, 2)])
@pytest.mark.parametrize("include_zero", [False, True])
def test_lazy_chunks_and_null_groups_equal_independent_oracle(engine, weight, domain, include_zero):
    source = frame([(0, 4, 2), (1, 6, 3), (2, 5, 3), (5, 7, 3), (-9, -9, 99), (-2, 9, 0)])
    source = source.with_columns(pl.Series("resource", ["b", None, "b", "b", "a", None]))
    chunked = pl.DataFrame(
        [
            pl.concat([column[:cut], column[cut:]], rechunk=False)
            for cut, column in zip([1, 3, 2, 4], source, strict=True)
        ]
    )
    assert [column.n_chunks() for column in chunked] == [2, 2, 2, 2]
    options = {"weight": weight, "include_zero": include_zero}
    if domain is not None:
        options.update(domain_start=domain[0], domain_end=domain[1])
    for by in ([], ["resource"]):
        query = pi.coverage_profile(chunked.lazy(), by=by, **options)
        actual = query.collect(engine=engine)
        assert_frame_equal(actual, pi.coverage_profile(source, by=by, **options))
        if not by:
            rows = [(s, e, 1 if weight is None else q) for s, e, q, _ in source.iter_rows()]
            assert actual.rows() == oracle(rows, domain, include_zero)
        else:
            expected = []
            for key in dict.fromkeys(source["resource"]):
                rows = [
                    (s, e, 1 if weight is None else q)
                    for s, e, q, resource in source.iter_rows()
                    if resource == key
                ]
                expected.extend((key, *segment) for segment in oracle(rows, domain, include_zero))
            assert actual.rows() == expected
        assert_frame_equal(
            actual,
            pi.coverage_profile(chunked.rechunk().lazy(), by=by, **options).collect(engine=engine),
        )


TRANSFORMS = {
    "coordinate_filter": lambda query: query.filter(pl.col("start") >= 2),
    "load_filter": lambda query: query.filter(pl.col("load") >= 3),
    "projection": lambda query: query.select("load"),
    "head": lambda query: query.head(1),
    "slice": lambda query: query.slice(1, 1),
    "length": lambda query: query.select(pl.len().alias("segments")),
}


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
def test_downstream_operations_apply_to_complete_profile(engine, transform):
    source = frame([(0, 10, 1), (2, 4, 2), (4, 7, 2), (6, 8, 0)])
    # A source column named load must not intercept a filter on the output load.
    source = source.with_columns(pl.lit(-99).alias("load"))
    query = pi.coverage_profile(source.lazy(), weight="q")
    expected = pi.coverage_profile(source, weight="q")
    assert_frame_equal(transform(query).collect(engine=engine), transform(expected))


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
@pytest.mark.parametrize(
    "bad_row,match",
    [((9, 8, 0), "index 2"), ((9, 9, -1), "index 2"), ((None, 9, 0), "null endpoints")],
)
def test_downstream_operations_cannot_hide_invalid_input_rows(engine, transform, bad_row, match):
    source = frame([(0, 5, 2), (2, 4, 3), bad_row])
    query = pi.coverage_profile(source.lazy(), weight="q", domain_start=0, domain_end=5)
    # Schema inspection must not execute the native solve or inspect row values.
    assert query.collect_schema()["load"] == pl.Int128
    with pytest.raises(pl.exceptions.ComputeError, match=match):
        transform(query).collect(engine=engine)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_upstream_filter_intentionally_changes_the_instance(engine):
    source = frame([(0, 4, 2), (1, 5, 3), (100, 99, -1)])
    query = pi.coverage_profile(source.lazy().filter(pl.col("start") < 10), weight="q")
    expected = pi.coverage_profile(source.head(2), weight="q")
    assert_frame_equal(query.collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_aliases_and_group_keys_sharing_source_columns(engine):
    source = pl.DataFrame({"s": [0, 0, 2], "e": [4, 3, 5], "q": [1, 2, 3]})
    prepared = source.lazy().with_columns((pl.col("e") + 1).alias("finish"))
    query = pi.coverage_profile(prepared, start="s", end="finish", weight="q", by=["q", "s"])
    assert query.collect_schema().names() == ["q", "s", "start", "end", "load"]
    expected = pi.coverage_profile(
        source.with_columns((pl.col("e") + 1).alias("finish")),
        start="s",
        end="finish",
        weight="q",
        by=["q", "s"],
    )
    assert_frame_equal(query.collect(engine=engine), expected)
    empty_intervals = pi.coverage_profile(
        source.lazy(),
        start="s",
        end="s",
        by="s",
        domain_start=-1,
        domain_end=6,
        include_zero=True,
    )
    assert empty_intervals.collect(engine=engine).rows() == [(0, -1, 6, 0), (2, -1, 6, 0)]
    literal_names = source.rename({"s": "^s$", "e": "*", "q": "^q$"})
    options = {"start": "^s$", "end": "*", "weight": "^q$", "by": "^q$"}
    assert_frame_equal(
        pi.coverage_profile(literal_names.lazy(), **options).collect(engine=engine),
        pi.coverage_profile(literal_names, **options),
    )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_deferred_arguments_are_snapshots_of_mutable_inputs(engine):
    source = frame([(0, 4, 2), (2, 6, 3)]).with_columns(
        pl.Series("resource", ["a", "b"]), pl.Series("other", [1, 2])
    )
    keys = ["resource"]
    left, right = pl.Series([-1], dtype=pl.Int64), pl.Series([7], dtype=pl.Int64)
    query = pi.coverage_profile(
        source.lazy(),
        weight="q",
        by=keys,
        domain_start=left,
        domain_end=right,
        include_zero=True,
    )
    expected = pi.coverage_profile(
        source, weight="q", by="resource", domain_start=-1, domain_end=7, include_zero=True
    )
    keys.append("other")
    left[0] = -10
    right[0] = 100
    assert query.collect_schema() == expected.schema
    assert_frame_equal(query.collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("format_name", ["parquet", "ipc"])
@pytest.mark.parametrize("weight", [None, "q"])
def test_partitioned_lazy_scans_solve_whole_groups(tmp_path, engine, format_name, weight):
    count = 257
    rows = [(i, i + 2, i % 5) for i in range(count)] + [(0, count + 1, 7)]
    source = frame(rows).with_columns(
        pl.Series("resource", [(["a", None, "b"])[i % 3] for i in range(count)] + ["a"])
    )
    paths = []
    for index, part in enumerate((source[:85], source[85:170], source[170:])):
        path = tmp_path / f"part-{index}.{format_name}"
        if format_name == "parquet":
            part.write_parquet(path, row_group_size=17)
        else:
            part.write_ipc(path, record_batch_size=17)
        paths.append(path)
    scan = pl.scan_parquet(paths) if format_name == "parquet" else pl.scan_ipc(paths)
    options = {"weight": weight, "by": "resource", "include_zero": True}
    actual = pi.coverage_profile(scan, **options).collect(engine=engine)
    assert_frame_equal(actual, pi.coverage_profile(source, **options))
    expected = []
    for key in dict.fromkeys(source["resource"]):
        group_rows = [
            (s, e, 1 if weight is None else q)
            for s, e, q, resource in source.iter_rows()
            if resource == key
        ]
        expected.extend((key, *segment) for segment in oracle(group_rows, include_zero=True))
    assert actual.rows() == expected


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_deferred_temporal_bounds_and_exact_int128_loads(engine):
    dtype = pl.Datetime("ns", "Europe/Helsinki")
    t = 1729990800000000001
    source = pl.DataFrame(
        {
            "start": pl.Series([t - 2, t, t], dtype=dtype),
            "end": pl.Series([t, t + 2, t + 2], dtype=dtype),
            "q": pl.Series([2**64 - 1] * 3, dtype=pl.UInt64),
        }
    )
    options = {
        "weight": "q",
        "domain_start": pl.Series([t - 1], dtype=dtype),
        "domain_end": pl.Series([t + 1], dtype=dtype),
    }
    query = pi.coverage_profile(source.lazy(), **options)
    assert query.collect_schema() == {"start": dtype, "end": dtype, "load": pl.Int128}
    actual = query.collect(engine=engine)
    assert actual.select(pl.col("start", "end").to_physical(), "load").rows() == [
        (t - 1, t, 2**64 - 1),
        (t, t + 1, 2 * (2**64 - 1)),
    ]
    assert_frame_equal(actual, pi.coverage_profile(source, **options))
