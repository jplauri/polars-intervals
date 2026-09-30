"""Independent small graph/cell checks for the native-Polars competitor."""

import importlib
import random
import sys

import polars as pl
import pytest
from polars.testing import assert_frame_equal

from benchmarks.interval_geometry_native import native_cluster, native_gaps, native_merge
from tests.test_interval_geometry import cells_oracle, cluster_oracle


def test_native_random_original_problem_oracles():
    rng = random.Random(821)
    for _ in range(20):
        rows = [
            (*sorted((rng.randrange(-5, 8), rng.randrange(-5, 8))), rng.choice([None, "a", "b"]))
            for _ in range(rng.randrange(13))
        ]
        df = pl.DataFrame(
            rows, schema={"start": pl.Int64, "end": pl.Int64, "g": pl.String}, orient="row"
        )
        for grouped in (False, True):
            groups = list(dict.fromkeys(g for _, _, g in rows)) if grouped else [None]
            geometry = lambda key, rows=rows, grouped=grouped: [
                (s, e) for s, e, g in rows if not grouped or key == g
            ]
            by = "g" if grouped else None
            for touching in (False, True):
                per_group = {g: iter(cluster_oracle(geometry(g), touching)) for g in groups}
                expected = [next(per_group[g if grouped else None]) for _, _, g in rows]
                for source in (df, df.lazy()):
                    output = native_cluster(source, by=by, include_touching=touching)
                    if isinstance(output, pl.LazyFrame):
                        output = output.collect(engine="streaming")
                    assert output["cluster"].to_list() == expected
                    assert output["cluster"].dtype == pl.UInt32
            for domain in (None, (-3, 5), (0, 0), (20, 25)):
                expected = [
                    (*((g,) if grouped else ()), *segment)
                    for g in groups
                    for segment in cells_oracle(geometry(g), domain)
                ]
                operation = native_merge if domain is None else native_gaps
                options = (
                    {} if domain is None else {"domain_start": domain[0], "domain_end": domain[1]}
                )
                eager = operation(df, by=by, **options)
                lazy = operation(df.lazy(), by=by, **options)
                assert eager.rows() == expected
                assert_frame_equal(eager, lazy.collect(engine="streaming"))


@pytest.mark.parametrize(
    "dtype", [pl.Int8, pl.UInt64, pl.Date, pl.Datetime("ns", "Europe/Helsinki")]
)
def test_native_dtype_names_chunks_and_optimizer(dtype):
    base = 2**63 + 5 if dtype == pl.UInt64 else 2**53 + 3 if isinstance(dtype, pl.Datetime) else 0
    df = pl.DataFrame(
        {
            "*": [base, base + 2, base + 9],
            "^e$": [base + 3, base + 5, base + 9],
            "_g": [None, None, "a"],
        }
    ).with_columns(pl.selectors.by_name("*", "^e$").cast(dtype))
    # Literal selectors are needed: wildcard/regex-looking names are legal.
    df = pl.concat([df.head(1), df.tail(2)], rechunk=False)
    options = {"start": "*", "end": "^e$", "by": "_g"}
    merged = native_merge(df, **options)
    assert merged.schema == {"_g": pl.String, "start": dtype, "end": dtype}
    assert merged.select(pl.col("start", "end").to_physical()).rows() == [(base, base + 5)]
    lazy = native_merge(df.lazy(), **options)
    assert (
        lazy.filter(pl.col("start") > pl.lit(pl.Series([base], dtype=dtype)).first())
        .collect()
        .height
        == 0
    )
    assert_frame_equal(lazy.select("end").head(1).collect(), merged.select("end").head(1))
    bounds = {
        "domain_start": pl.Series([base], dtype=dtype),
        "domain_end": pl.Series([base + 10], dtype=dtype),
    }
    gaps = native_gaps(df, **options, **bounds)
    assert gaps.select(pl.col("start", "end").to_physical()).rows() == [
        (base + 5, base + 10),
        (base, base + 10),
    ]


@pytest.mark.parametrize("operation", [native_cluster, native_merge, native_gaps])
def test_native_lazy_validation_and_schema_only_planning(operation):
    calls = []
    df = pl.DataFrame({"start": [0, 20], "end": [2, 19]})
    source = df.lazy().map_batches(
        lambda frame: calls.append(frame.height) or frame, schema=df.schema, streamable=False
    )
    options = {"domain_start": 0, "domain_end": 0} if operation == native_gaps else {}
    query = operation(source, **options)
    query.collect_schema()
    query.explain()
    assert calls == []
    with pytest.raises(pl.exceptions.ComputeError, match="index 1"):
        query.head(1).collect(engine="streaming")


@pytest.mark.parametrize("operation", [native_cluster, native_merge, native_gaps])
def test_native_null_endpoints_validate_before_pruning(operation):
    df = pl.DataFrame(
        {"start": [0, None], "end": [0, 1]}, schema={"start": pl.Int64, "end": pl.Int64}
    )
    options = {"domain_start": 0, "domain_end": 0} if operation == native_gaps else {}
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        operation(df, **options)
    query = operation(df.lazy(), **options)
    query.collect_schema()
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        query.head(1).collect(engine="streaming")


def test_native_reversed_domain_validation_is_deferred():
    for rows in ([], [(0, 1)]):
        df = pl.DataFrame(rows, schema={"start": pl.Int64, "end": pl.Int64}, orient="row")
        with pytest.raises(pl.exceptions.ComputeError, match="domain start"):
            native_gaps(df, domain_start=2, domain_end=1)
        query = native_gaps(df.lazy(), domain_start=2, domain_end=1)
        query.collect_schema()
        query.explain()
        with pytest.raises(pl.exceptions.ComputeError, match="domain start"):
            query.collect(engine="streaming")


def test_native_rejects_invalid_options():
    df = pl.DataFrame({"start": [0], "end": [1]})
    with pytest.raises(TypeError):
        native_cluster(df, include_touching=1)
    for value in (True, None, pl.Series([0, 1]), pl.Series([None], dtype=pl.Int64)):
        with pytest.raises((TypeError, ValueError, pl.exceptions.PolarsError)):
            native_gaps(df, domain_start=value, domain_end=2)
    with pytest.raises(pl.exceptions.InvalidOperationError):
        native_gaps(df, domain_start=pl.Series([0], dtype=pl.Int32), domain_end=2)


def test_native_cluster_window_keys_keep_polars_grouping_contract():
    df = pl.DataFrame(
        {
            "start": [2, 0, 2],
            "end": [4, 1, 5],
            "key": [b"a", b"b", b"a"],
            "float": [None, 0.5, None],
        }
    )
    for by in ("start", "key", "float"):
        assert native_cluster(df, by=by)["cluster"].to_list() == [0, 0, 0]


def test_native_multiple_typed_nullable_keys_and_group_universe():
    df = pl.DataFrame(
        {
            "start": [3, 0, 0, 4],
            "end": [3, 2, 0, 6],
            "flag": [None, True, None, True],
            "day": pl.Series([1, 2, 1, 2], dtype=pl.Date),
        }
    )
    options = {"by": ["flag", "day"], "domain_start": 0, "domain_end": 8}
    expected = [(None, 1, 0, 8), (True, 2, 2, 4), (True, 2, 6, 8)]
    for source in (df, df.lazy()):
        result = native_gaps(source, **options)
        if isinstance(result, pl.LazyFrame):
            result = result.collect(engine="streaming")
        assert result.schema == {
            "flag": pl.Boolean,
            "day": pl.Date,
            "start": pl.Int64,
            "end": pl.Int64,
        }
        assert result.with_columns(pl.col("day").to_physical()).rows() == expected


def test_native_cluster_invalid_row_index_is_local_to_group():
    df = pl.DataFrame(
        {"start": [0, 0, 1, 2, 3], "end": [1, 1, 3, 1, 4], "g": ["a", None, "a", None, "a"]}
    )
    with pytest.raises(pl.exceptions.ComputeError, match="interval at index 1 has start greater"):
        native_cluster(df, by="g")
    query = native_cluster(df.lazy(), by="g")
    with pytest.raises(pl.exceptions.ComputeError, match="interval at index 1 has start greater"):
        query.collect(engine="streaming")


@pytest.mark.parametrize(
    "runner,option,value",
    [
        (runner, option, value)
        for runner in ("interval_geometry", "interval_geometry_core")
        for option, value in (("--sizes", "-1"), ("--samples", "0"), ("--warmups", "0"))
    ]
    + [
        ("interval_geometry", "--cases", "-1"),
        ("interval_geometry_core", "--dtypes", "i128"),
        ("interval_geometry_core", "--seeds", "-1"),
        ("interval_geometry_core", "--operations", "unknown"),
    ],
)
def test_geometry_runners_reject_invalid_settings(monkeypatch, tmp_path, runner, option, value):
    module = importlib.import_module(f"benchmarks.{runner}")
    monkeypatch.setattr(sys, "argv", [runner, "--output", str(tmp_path / "run"), option, value])
    with pytest.raises(SystemExit) as error:
        module.main()
    assert error.value.code == 2
    assert not list(tmp_path.iterdir())
