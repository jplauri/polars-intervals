"""Evaluation identity and payload regressions for the two-input boundary."""

import polars as pl
import polars_intervals as pi
import pytest
from polars.testing import assert_frame_equal

from .test_coverage_stats import STATS, assert_stats, frame, oracle


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_payload_and_geometry_share_the_same_query_evaluation(engine):
    queries = frame([(0, 100), (2, 5), (4, 4)]).with_columns(
        pl.Series("id", [1, 1, 1]),
        pl.Series("payload", [[0], None, []]),
        pl.lit(0).cast(pl.Int64).alias("generation"),
    )
    queries = queries.select("payload", "end", "generation", "id", "start")
    sources = frame([(200 * i + 1, 200 * i + i + 2) for i in range(32)])
    calls = []

    def evaluate_query(batch):
        generation = len(calls)
        calls.append(generation)
        return batch.with_columns(
            (pl.col("start", "end") + 200 * generation),
            pl.lit(generation, dtype=pl.Int64).alias("generation"),
        )

    query = queries.lazy().map_batches(
        evaluate_query,
        schema=queries.schema,
        predicate_pushdown=False,
        projection_pushdown=False,
        slice_pushdown=False,
        streamable=False,
    )
    plan = pi.coverage_stats(query, sources.lazy())
    assert plan.collect_schema() == {**queries.schema, **STATS}
    plan.explain()
    assert calls == []
    for _ in range(3):
        previous_calls = len(calls)
        result = plan.collect(engine=engine)
        assert len(calls) > previous_calls
        actual_queries = result.select(queries.columns)
        # A separate metadata evaluation would assign different coordinates and
        # generation to the rows used for calculation. No callback count is fixed.
        assert_stats(result, oracle(actual_queries, sources))
        assert result.columns == [*queries.columns, *STATS]
        assert result["id"].to_list() == [1, 1, 1]
        assert result["payload"].to_list() == [[0], None, []]


@pytest.mark.parametrize("engine", ["auto", "streaming"])
def test_repeated_collect_reads_the_current_source_evaluation(engine):
    queries = frame([(0, 10), (5, 10), (7, 7)])
    initial = frame([(1, 4)])
    current = {"frame": initial}
    calls = []

    def evaluate_source(_batch):
        calls.append(current["frame"].height)
        return current["frame"]

    source = initial.lazy().map_batches(
        evaluate_source,
        schema=initial.schema,
        predicate_pushdown=False,
        projection_pushdown=False,
        slice_pushdown=False,
        streamable=False,
    )
    plan = pi.coverage_stats(queries.lazy(), source)
    plan.collect_schema()
    plan.explain()
    assert calls == []
    for sources in (initial, frame([(1, 4), (6, 9), (6, 9)]), frame([])):
        current["frame"] = sources
        previous_calls = len(calls)
        assert_stats(plan.collect(engine=engine), oracle(queries, sources))
        assert len(calls) > previous_calls


@pytest.mark.parametrize(
    "qlazy,slazy", [(False, False), (False, True), (True, False), (True, True)]
)
@pytest.mark.parametrize("empty", [False, True])
def test_payload_order_empty_schema_and_irrelevant_weight_columns(qlazy, slazy, empty):
    queries = frame([(0, 10), (3, 3), (5, 10)]).with_columns(
        pl.Series("weight", [-100, None, 999]),
        pl.Series("nested", [{"values": [1, None]}, None, {"values": []}]),
        pl.Series("label", ["b", None, "a"], dtype=pl.Categorical),
    )
    queries = queries.select("label", "end", "weight", "nested", "start")
    if empty:
        queries = queries.head(0)
    sources = frame([(1, 7), (4, 9)]).with_columns(
        pl.Series("weight", [[-10], None]),
        pl.Series("overlap_count", [None, "ignored"]),
        pl.lit("ignored").alias("query_length"),
    )
    result = pi.coverage_stats(
        queries.lazy() if qlazy else queries,
        sources.lazy() if slazy else sources,
    )
    if isinstance(result, pl.LazyFrame):
        result = result.collect(engine="streaming")
    assert result.schema == {**queries.schema, **STATS}
    assert_frame_equal(result.select(queries.columns), queries)
    assert_stats(result, oracle(queries, sources))


@pytest.mark.parametrize("engine", ["auto", "streaming"])
@pytest.mark.parametrize("side", ["queries", "intervals"])
def test_explicit_upstream_transforms_define_validation_rows(side, engine):
    good = frame([(0, 10)])
    original = frame([(100, 99), (0, 1), (20, 19)])
    # The discarded invalid row is outside this explicitly evaluated operand.
    # The remaining bad row has index 1, rather than its original file index 2.
    bad = original.lazy().filter(pl.col("start") < 100)
    queries, intervals = (bad, good) if side == "queries" else (good, bad)
    with pytest.raises(pl.exceptions.ComputeError, match=rf"{side}.*index 1"):
        pi.coverage_stats(queries, intervals).collect(engine=engine)
    clean = original.lazy().filter(pl.col("start") == 0)
    queries, intervals = (clean, good) if side == "queries" else (good, clean)
    actual = pi.coverage_stats(queries, intervals).collect(engine=engine)
    assert_stats(
        actual,
        oracle(frame([(0, 1)]), good) if side == "queries" else oracle(good, frame([(0, 1)])),
    )
