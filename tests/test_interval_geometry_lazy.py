"""Optimizer and whole-collection regressions for the real compiled operations."""

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .dtypes import ENDPOINT_DTYPES
from .test_interval_geometry import cells_oracle, cluster_oracle, frame


def geometry(source, gap=False, **options):
    if gap:
        return pi.interval_gaps(source, domain_start=0, domain_end=12, **options)
    return pi.merge_intervals(source, **options)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("gap", [False, True])
def test_lazy_construction_explain_schema_and_repeated_collection(engine, gap):
    source = frame([(0, 4), (3, 7), (6, 10)])
    observed = []

    def record(batch):
        observed.append(batch.height)
        return batch

    lazy = source.lazy().map_batches(record, schema=source.schema, streamable=False)
    query = geometry(lazy, gap)
    assert isinstance(query, pl.LazyFrame)
    assert query.collect_schema() == source.schema
    assert query.select("end").collect_schema() == {"end": pl.Int64}
    assert query.explain()
    assert observed == []
    expected = geometry(source, gap)
    assert isinstance(expected, pl.DataFrame)
    assert_frame_equal(query.collect(engine=engine), expected)
    assert observed == [3]
    assert_frame_equal(query.collect(engine=engine), expected)
    assert observed == [3, 3]
    assert_frame_equal(source.lazy().pipe(geometry, gap).collect(engine=engine), expected)


TRANSFORMS = {
    "start_filter": lambda df: df.filter(pl.col("start") >= 1),
    "end_filter": lambda df: df.filter(pl.col("end") < 11),
    "only_end": lambda df: df.select("end"),
    "only_keys": lambda df: df.select("g"),
    "head": lambda df: df.head(1),
    "slice": lambda df: df.slice(1, 1),
    "count": lambda df: df.select(pl.len()),
    "sort": lambda df: df.sort("end", descending=True),
}


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("gap", [False, True])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
def test_downstream_operations_cannot_change_interval_instance(engine, gap, transform):
    source = frame([(0, 2), (4, 6), (1, 5), (8, 9)]).with_columns(
        pl.Series("g", ["a", "a", "a", None])
    )
    query = transform(geometry(source.lazy(), gap, by="g"))
    expected = transform(geometry(source, gap, by="g"))
    assert_frame_equal(query.collect(engine=engine), expected)
    assert_frame_equal(
        query.collect(engine=engine, optimizations=pl.QueryOptFlags.none()), expected
    )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_union_filter_pushdown_regression(engine):
    source = frame([(0, 3), (2, 5)])
    result = pi.merge_intervals(source.lazy()).filter(pl.col("start") >= 1).collect(engine=engine)
    assert result.is_empty()
    changed_instance = pi.merge_intervals(source.lazy().filter(pl.col("start") >= 1)).collect(
        engine=engine
    )
    assert changed_instance.rows() == [(2, 5)]


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("over", [False, True])
def test_cluster_bridge_survives_downstream_metadata_filter_and_slice(engine, over):
    source = frame([(0, 4), (3, 7), (6, 10)]).with_columns(
        pl.Series("keep", [True, False, True]), pl.lit("a").alias("g")
    )
    expr = pi.cluster_intervals("start", "end")
    if over:
        expr = expr.over("g")
    query = source.lazy().with_columns(expr.alias("cluster"))
    assert query.filter(pl.col("keep")).collect(engine=engine)["cluster"].to_list() == [0, 0]
    assert source.lazy().filter(pl.col("keep")).with_columns(expr.alias("cluster")).collect(
        engine=engine
    )["cluster"].to_list() == [0, 1]
    bridge_last = frame([(0, 2), (4, 6), (1, 5)]).with_columns(pl.lit("a").alias("g"))
    assert bridge_last.lazy().with_columns(expr.alias("cluster")).head(2).collect(engine=engine)[
        "cluster"
    ].to_list() == [0, 0]


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("gap", [False, True])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
@pytest.mark.parametrize("bad", [(100, 99), (None, 99)])
def test_later_invalid_rows_not_hidden_by_optimizer(engine, gap, transform, bad):
    source = frame([(0, 3), (5, 6), bad]).with_columns(pl.Series("g", ["b", "a", "b"]))
    query = transform(geometry(source.lazy(), gap, by="g"))
    assert query.collect_schema()
    with pytest.raises(pl.exceptions.ComputeError, match="index 2|null endpoints"):
        query.collect(engine=engine)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_lazy_cluster_does_not_execute_at_construction_and_validates_late_rows(engine):
    source = frame([(0, 3), (2, 5), (100, 99)]).with_columns(pl.Series("keep", [True, True, False]))
    seen = []

    def record(batch):
        seen.append(batch.height)
        return batch

    query = (
        source.lazy()
        .map_batches(record, schema=source.schema)
        .with_columns(pi.cluster_intervals("start", "end").alias("cluster"))
    )
    assert query.collect_schema()["cluster"] == pl.UInt32
    assert query.explain()
    assert not seen
    for narrowed in (query.head(1), query.filter(pl.col("keep"))):
        with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
            narrowed.collect(engine=engine)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_lazy_endpoint_metadata_and_zero_rows(engine, dtype):
    source = frame([(0, 2), (2, 4)], dtype).with_columns(
        pl.Series("g", [1, None], dtype=pl.Datetime("ns", "UTC"))
    )
    for data in (source, source.clear()):
        for by in (None, "g"):
            for operation, options in [
                (pi.merge_intervals, {}),
                (
                    pi.interval_gaps,
                    {
                        "domain_start": pl.Series([0], dtype=dtype),
                        "domain_end": pl.Series([4], dtype=dtype),
                    },
                ),
            ]:
                expected = operation(data, by=by, **options)
                query = operation(data.lazy(), by=by, **options)
                assert query.collect_schema() == expected.schema
                assert_frame_equal(query.collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_multichunk_columns_windows_and_group_aggregation(engine):
    rows = [(0, 4), (2, 3), (3, 7), (9, 9), (6, 10), (10, 12)]
    source = frame(rows).with_columns(pl.Series("g", ["a", None, "a", "a", "a", None]))
    chunked = pl.DataFrame(
        [
            pl.concat([col[:cut], col[cut:]], rechunk=False)
            for col, cut in zip(source, [1, 3, 2], strict=True)
        ]
    )
    assert all(col.n_chunks() == 2 for col in chunked)
    for touching in (False, True):
        expr = pi.cluster_intervals("start", "end", include_touching=touching).alias("cluster")
        assert chunked.lazy().select(expr).collect(
            engine=engine
        ).to_series().to_list() == cluster_oracle(rows, touching)
        assert_frame_equal(
            chunked.lazy().with_columns(expr.over("g")).collect(engine=engine),
            source.with_columns(expr.over("g")),
        )
        assert_frame_equal(
            chunked.lazy().group_by("g", maintain_order=True).agg(expr).collect(engine=engine),
            source.group_by("g", maintain_order=True).agg(expr),
        )
    for gap in (False, True):
        for by in (None, "g"):
            assert_frame_equal(
                geometry(chunked.lazy(), gap, by=by).collect(engine=engine),
                geometry(source, gap, by=by),
            )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_parquet_files_and_row_groups_form_one_instance(tmp_path, engine):
    rows = [(i, i + 2) for i in range(257)] + [(0, 258)]
    source = frame(rows).with_columns(
        pl.Series("g", [(["b", None, "a"])[i % 3] for i in range(257)] + ["a"])
    )
    paths = []
    for number, part in enumerate((source[:85], source[85:170], source[170:])):
        path = tmp_path / f"part-{number}.parquet"
        part.write_parquet(path, row_group_size=17)
        paths.append(path)
    scan = pl.scan_parquet(paths)
    for touching in (False, True):
        expr = pi.cluster_intervals("start", "end", include_touching=touching).alias("cluster")
        assert scan.select(expr).collect(engine=engine).to_series().to_list() == cluster_oracle(
            rows, touching
        )
        assert_frame_equal(
            scan.with_columns(expr.over("g")).collect(engine=engine),
            source.with_columns(expr.over("g")),
        )
    for by in (None, "g"):
        for operation, options in [
            (pi.merge_intervals, {}),
            (pi.interval_gaps, {"domain_start": -1, "domain_end": 260}),
        ]:
            result = operation(scan, by=by, **options).collect(engine=engine)
            assert_frame_equal(result, operation(source, by=by, **options))
            expected = []
            groups = (
                [(None, rows)]
                if by is None
                else [
                    (key, [(s, e) for s, e, g in source.iter_rows() if g == key])
                    for key in dict.fromkeys(source["g"])
                ]
            )
            for key, group in groups:
                expected.extend(
                    (*(() if by is None else (key,)), *pair)
                    for pair in cells_oracle(group, (-1, 260) if options else None)
                )
            assert result.rows() == expected


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_domain_and_group_arguments_are_frozen_when_plan_is_built(engine):
    source = frame([(0, 3), (5, 6)]).with_columns(
        pl.Series("g", ["a", "b"]), pl.lit(1).alias("other")
    )
    left, right = pl.Series([0]), pl.Series([10])
    keys = ["g"]
    query = pi.interval_gaps(source.lazy(), by=keys, domain_start=left, domain_end=right)
    expected = pi.interval_gaps(source, by="g", domain_start=0, domain_end=10)
    left[0], right[0] = -100, 100
    keys.append("other")
    assert_frame_equal(query.collect(engine=engine), expected)


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_scan_validation_errors_use_evaluated_original_indices(tmp_path, engine):
    source = frame([(0, 2), (3, 5), (9, 8)]).with_columns(pl.Series("g", [None, "a", None]))
    path = tmp_path / "invalid.parquet"
    source.write_parquet(path, row_group_size=1)
    for domain in ((0, 0), (100, 200)):
        query = pi.interval_gaps(
            pl.scan_parquet(path), by="g", domain_start=domain[0], domain_end=domain[1]
        )
        with pytest.raises(pl.exceptions.ComputeError, match="index 2"):
            query.head(1).collect(engine=engine)
