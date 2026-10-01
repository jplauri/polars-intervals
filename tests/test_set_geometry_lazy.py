"""Deferred two-input execution, optimizer barriers and partition regressions."""

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .test_set_geometry import OPERATIONS, frame, grouped_oracle


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize(
    "left_lazy,right_lazy", [(False, False), (True, False), (False, True), (True, True)]
)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_all_frame_modes_defer_both_sources_and_preserve_inputs(
    operation, left_lazy, right_lazy, engine
):
    left = frame([(0, 5), (4, 10), (12, 15)])
    right = frame([(2, 3), (6, 8), (10, 13)])
    snapshots = left.clone(), right.clone()
    observed = [[], []]

    def source(df, side, lazy):
        def record(batch):
            observed[side].append(batch.height)
            return batch

        return df.lazy().map_batches(record, schema=df.schema, streamable=False) if lazy else df

    result = operation(source(left, 0, left_lazy), source(right, 1, right_lazy))
    expected = operation(left, right)
    assert_frame_equal(left, snapshots[0])
    assert_frame_equal(right, snapshots[1])
    if left_lazy or right_lazy:
        assert isinstance(result, pl.LazyFrame)
        assert result.collect_schema() == expected.schema
        assert result.explain()
        assert observed == [[], []]
        for _ in range(2):
            assert_frame_equal(result.collect(engine=engine), expected)
        assert bool(observed[0]) == left_lazy
        assert bool(observed[1]) == right_lazy
    else:
        assert isinstance(result, pl.DataFrame)
        assert_frame_equal(result, expected)
    piped = left.lazy().pipe(operation, right.lazy())
    assert_frame_equal(piped.collect(engine=engine), expected)


TRANSFORMS = {
    "start_filter": lambda df: df.filter(pl.col("start") >= 2),
    "end_filter": lambda df: df.filter(pl.col("end") <= 8),
    "end_only": lambda df: df.select("end"),
    "keys_only": lambda df: df.select("key"),
    "count": lambda df: df.select(pl.len()),
    "head": lambda df: df.head(1),
    "tail": lambda df: df.tail(1),
    "slice": lambda df: df.slice(1, 1),
}


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("transform", TRANSFORMS.values(), ids=TRANSFORMS)
def test_downstream_transforms_keep_both_full_operands(operation, engine, transform):
    left = frame([(0, 2), (1, 10), (12, 15), (0, 0)]).with_columns(
        pl.Series("key", ["a", "a", None, "empty"])
    )
    right = frame([(6, 8), (0, 4), (13, 14)]).with_columns(pl.Series("key", ["a", "a", None]))
    expected = transform(grouped_oracle(left, right, ["key"], operation))
    result = transform(operation(left.lazy(), right.lazy(), by="key"))
    assert_frame_equal(result.collect(engine=engine), expected)
    assert_frame_equal(
        result.collect(engine=engine, optimizations=pl.QueryOptFlags.none()), expected
    )


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_explicit_filter_and_slice_regressions(engine):
    left, right = frame([(0, 10)]).lazy(), frame([(2, 4)]).lazy()
    assert pi.subtract_intervals(left, right).filter(pl.col("start") >= 4).collect(
        engine=engine
    ).rows() == [(4, 10)]
    assert pi.intersect_intervals(left, right).filter(pl.col("start") >= 2).collect(
        engine=engine
    ).rows() == [(2, 4)]
    assert (
        pi.intersect_intervals(left.filter(pl.col("start") >= 2), right)
        .collect(engine=engine)
        .is_empty()
    )
    right = frame([(6, 8), (0, 4)]).lazy()
    assert pi.subtract_intervals(left, right).head(1).collect(engine=engine).rows() == [(4, 6)]
    left = frame([(0, 2), (1, 10)]).lazy()
    right = frame([(4, 6)]).lazy()
    assert pi.subtract_intervals(left, right).head(1).collect(engine=engine).rows() == [(0, 4)]
    assert pi.intersect_intervals(left, right).head(1).collect(engine=engine).rows() == [(4, 6)]


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_multichunk_and_scan_sources_do_not_pair_partitions(operation, engine, tmp_path):
    left = frame([(0, 0), (2, 8), (0, 10), (12, 20), (0, 30), (21, 25)]).with_columns(
        pl.Series("key", ["early", None, "a", "a", "early", None])
    )
    right = frame([(100, 101), (3, 7), (15, 18), (1, 29)]).with_columns(
        pl.Series("key", ["absent", None, "a", "early"])
    )
    expected = grouped_oracle(left, right, ["key"], operation)
    chunks = []
    scans = []
    for side, source, lengths in (("left", left, [0, 2, 1, 3]), ("right", right, [1, 0, 3])):
        parts, paths, offset = [], [], 0
        for i, length in enumerate(lengths):
            part = source.slice(offset, length)
            offset += length
            parts.append(part)
            path = tmp_path / f"{side}-{i}.parquet"
            part.write_parquet(path, row_group_size=1 if side == "left" else 2)
            paths.append(path)
        chunks.append(pl.concat(parts, rechunk=False))
        scans.append(pl.scan_parquet(paths))
    assert chunks[0].n_chunks() > 1 and chunks[1].n_chunks() > 1
    assert_frame_equal(operation(*chunks, by="key"), expected)
    assert_frame_equal(
        operation(*(df.lazy() for df in chunks), by="key").collect(engine=engine), expected
    )
    assert_frame_equal(operation(*scans, by="key").collect(engine=engine), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_shared_source_different_branches_and_same_input(operation, engine):
    source = frame([(0, 10), (2, 4), (6, 8)])
    lazy = source.lazy()
    expected = operation(source, source)
    assert_frame_equal(operation(lazy, lazy).collect(engine=engine), expected)
    left = lazy.filter(pl.col("start") == 0)
    right = lazy.filter(pl.col("start") > 0)
    expected = operation(source.head(1), source.tail(2))
    assert_frame_equal(operation(left, right).collect(engine=engine), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
def test_freeze_keys_and_temporary_name_collisions(operation):
    left = frame([(0, 10), (0, 6)]).with_columns(
        pl.Series("__pi_left", ["a", "b"]),
        pl.Series("__pi_row", [0, 1]),
        pl.Series("__pi_left_", [True, False]),
        pl.Series("__pi_row_", [1, 2]),
    )
    right = left.with_columns(pl.col("start") + 2, pl.col("end") - 2)
    keys = ["__pi_left", "__pi_left_", "__pi_row", "__pi_row_"]
    expected = grouped_oracle(left, right, keys, operation)
    query = operation(left.lazy(), right.lazy(), by=keys)
    keys[:] = ["nonexistent"]
    assert_frame_equal(query.collect(), expected)


@pytest.mark.parametrize("operation", OPERATIONS)
@pytest.mark.parametrize("side", ["left", "right"])
@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_late_invalid_scan_rows_cannot_be_sliced_away(operation, side, engine, tmp_path):
    good = frame([(0, 10)])
    bad = frame([(1, 2), (4, 5), (8, 7)])
    path = tmp_path / "bad.parquet"
    bad.write_parquet(path, row_group_size=1)
    scan = pl.scan_parquet(path)
    left, right = (scan, good) if side == "left" else (good, scan)
    with pytest.raises(pl.exceptions.ComputeError, match=rf"{side}.*index 2"):
        operation(left, right).head(1).select("end").collect(engine=engine)


@pytest.mark.parametrize("operation", OPERATIONS)
def test_diagnostics_relative_to_evaluated_operand(operation):
    bad = frame([(0, 1), (2, 3), (9, 8)]).lazy().slice(1)
    with pytest.raises(pl.exceptions.ComputeError, match="right.*index 1"):
        operation(frame([]), bad).collect()
