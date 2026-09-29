"""Check the native benchmark competitor against original-row membership."""

import importlib
import json
import random
import sys
from pathlib import Path
from types import SimpleNamespace

import polars as pl
import pytest

from benchmarks.coverage_profile_native import native_profile
from tests.test_coverage_profile import oracle


@pytest.mark.parametrize("count_units", [False, True])
def test_native_oracle(count_units):
    rng = random.Random(52)
    for _ in range(25):
        rows = [
            (
                *sorted((rng.randrange(-5, 6), rng.randrange(-5, 6))),
                rng.randrange(4),
                rng.choice([None, "x", "y"]),
            )
            for _ in range(rng.randrange(12))
        ]
        df = pl.DataFrame(
            rows,
            schema={"start": pl.Int64, "end": pl.Int64, "q": pl.Int64, "g": pl.String},
            orient="row",
        )
        for domain in (None, (-4, 3), (0, 0), (7, 8)):
            for zero in (False, True):
                for weight in (None, "q"):
                    options = {"include_zero": zero, "weight": weight, "count_units": count_units}
                    if domain:
                        options.update(domain_start=domain[0], domain_end=domain[1])
                    expected = oracle(
                        [(s, e, 1 if weight is None else q) for s, e, q, _ in rows], domain, zero
                    )
                    assert native_profile(df, **options).rows() == expected
                    expected = [
                        (g, *segment)
                        for g in dict.fromkeys(row[3] for row in rows)
                        for segment in oracle(
                            [
                                (s, e, 1 if weight is None else q)
                                for s, e, q, key in rows
                                if key == g
                            ],
                            domain,
                            zero,
                        )
                    ]
                    assert native_profile(df, by="g", **options).rows() == expected


def test_native_wide_and_temporal():
    maximum = 2**64 - 1
    df = pl.DataFrame(
        {"start": [0, 0, 2], "end": [2, 2, 4], "q": pl.Series([maximum] * 3, dtype=pl.UInt64)}
    )
    assert native_profile(df, weight="q").rows() == [(0, 2, 2 * maximum), (2, 4, maximum)]
    for dtype in (pl.Date, pl.Datetime("ns", "Europe/Helsinki"), pl.UInt64):
        typed = df.with_columns(pl.col("start", "end").cast(dtype))
        result = native_profile(
            typed,
            weight="q",
            domain_start=pl.Series([1], dtype=dtype),
            domain_end=pl.Series([3], dtype=dtype),
        )
        assert result.schema == {"start": dtype, "end": dtype, "load": pl.Int128}
        assert result.select(pl.col("start", "end").to_physical(), "load").rows() == [
            (1, 2, 2 * maximum),
            (2, 3, maximum),
        ]


def test_native_scalar_columns_after_zero_filtering():
    # Pinned Polars keeps literal columns as scalar columns. Clipping must
    # broadcast them after a quantity filter, including with an explicit domain.
    df = pl.DataFrame({"q": [0, 1, 2, 3]}).select(
        pl.lit(0).alias("start"), pl.lit(4).alias("end"), "q"
    )
    assert native_profile(df, weight="q").rows() == [(0, 4, 6)]
    assert native_profile(df, weight="q", domain_start=1, domain_end=3).rows() == [(1, 3, 6)]


@pytest.mark.parametrize(
    "runner,option,value",
    [
        (runner, option, value)
        for runner in ("coverage_profile", "coverage_profile_core")
        for option, value in (("--sizes", "-1"), ("--samples", "0"), ("--warmups", "0"))
    ]
    + [
        ("coverage_profile", "--cases", "-1"),
        ("coverage_profile", "--cases", "18"),
        ("coverage_profile_core", "--dtypes", "i32"),
        ("coverage_profile_core", "--weight-dtypes", "i32"),
        ("coverage_profile_core", "--seeds", "-1"),
        ("coverage_profile_core", "--seeds", str(2**64)),
        ("coverage_profile_core", "--sizes", "1,bad"),
    ],
)
def test_runner_rejects_invalid_options(monkeypatch, tmp_path, runner, option, value):
    monkeypatch.syspath_prepend(str(Path(__file__).parent))
    module = importlib.import_module(runner)
    monkeypatch.setattr(sys, "argv", [runner, "--output", str(tmp_path / "raw.csv"), option, value])
    with pytest.raises(SystemExit) as error:
        module.main()
    assert error.value.code == 2
    assert not list(tmp_path.iterdir())


def test_core_runner_rejects_empty_measurement(monkeypatch, tmp_path):
    monkeypatch.syspath_prepend(str(Path(__file__).parent))
    module = importlib.import_module("coverage_profile_core")
    output = tmp_path / "raw.csv"
    monkeypatch.setattr(sys, "argv", ["coverage_profile_core", "--output", str(output)])
    monkeypatch.setattr(module, "environment", dict)
    monkeypatch.setattr(module, "command", lambda *args: "test")

    def empty_run(*args, **kwargs):
        output.write_text("n,method,sample,total_ns\n", encoding="utf-8")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr(module.subprocess, "run", empty_run)
    with pytest.raises(SystemExit) as error:
        module.main()
    assert error.value.code == 1
    metadata = json.loads(output.with_suffix(".metadata.json").read_text())
    assert metadata["returncode"] == 1
    assert "No samples emitted" in metadata["error"]
