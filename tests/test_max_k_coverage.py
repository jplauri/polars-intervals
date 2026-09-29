"""Native plugin integration and independent exhaustive union oracle."""

from datetime import UTC, date, datetime
from itertools import combinations
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

from .dtypes import ENDPOINT_DTYPES


def expr(k):
    return pi.max_k_coverage("start", "end", k=k).alias("selected")


def objective(rows, indices):
    total, right = 0, None
    for start, end in sorted(rows[i] for i in indices):
        total += max(0, end - max(start, right if right is not None else start))
        right = max(end, right if right is not None else end)
    return total, -len(indices)


def verify(frame, mask, k):
    rows = list(zip(frame["start"].to_physical(), frame["end"].to_physical(), strict=True))
    chosen = [i for i, selected in enumerate(mask) if selected]
    assert len(chosen) <= k
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(frame)
    expected = max(
        objective(rows, subset)
        for size in range(min(k, len(rows)) + 1)
        for subset in combinations(range(len(rows)), size)
    )
    assert objective(rows, chosen) == expected
    assert all(rows[i][0] < rows[i][1] for i in chosen)


@pytest.mark.parametrize("k", [0, 1, 2, 3, 8, 2**64 - 1])
@pytest.mark.parametrize(
    "rows",
    [
        [],
        [(5, 5)],
        [(2, 7)],
        [(0, 10), (-5, 4), (6, 15)],
        [(0, 10), (1, 11), (10, 18)],
        [(0, 10)] * 6,
        [(0, 20), (2, 18), (4, 16), (6, 14)],
        [(0, 5), (5, 10), (2, 2), (5, 5)],
        [(0, 10), (0, 4), (5, 10), (10, 12)],
    ],
)
def test_eager_lazy_filter_and_determinism(rows, k):
    frame = pl.DataFrame(rows, schema={"start": pl.Int64, "end": pl.Int64}, orient="row")
    mask = frame.select(expr(k)).to_series()
    verify(frame, mask, k)
    assert mask.equals(frame.select(expr(k)).to_series())
    assert mask.equals(frame.with_columns(expr(k))["selected"])
    assert mask.equals(frame.lazy().with_columns(expr(k)).collect()["selected"])
    assert frame.filter(expr(k)).equals(frame.filter(mask))
    assert frame.lazy().filter(expr(k)).collect().equals(frame.filter(mask))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
@pytest.mark.parametrize("k", [0, 1, 2, 8])
def test_logical_dtypes_multiple_chunks_and_shuffle(dtype, k):
    frame = pl.DataFrame({"start": [6, 0, 5, 0, 3], "end": [15, 10, 5, 4, 12]}).cast(dtype)
    chunked = pl.concat([frame[:2], frame[2:]], rechunk=False)
    assert chunked["start"].n_chunks() > 1
    mask = chunked.select(expr(k)).to_series()
    verify(chunked, mask, k)
    assert mask.equals(frame.select(expr(k)).to_series())
    empty = frame.clear().select(expr(k)).to_series()
    assert empty.dtype == pl.Boolean and empty.null_count() == 0 and len(empty) == 0


def test_group_windows_aggregates_and_expressions():
    frame = pl.DataFrame(
        {
            "group": ["a", "b", "a", "b", "a", "b"],
            "start": [0, 0, -5, 2, 6, 5],
            "end": [10, 10, 4, 2, 15, 12],
        }
    )
    window = frame.with_columns(expr(2).over("group"))
    for part in window.partition_by("group"):
        verify(part, part["selected"], 2)
    assert window.equals(frame.lazy().with_columns(expr(2).over("group")).collect())
    grouped = frame.group_by("group", maintain_order=True).agg(expr(2))
    for key, mask in grouped.iter_rows():
        part = frame.filter(pl.col("group") == key)
        verify(part, pl.Series(mask, dtype=pl.Boolean), 2)
    mask = frame.select(pi.max_k_coverage(pl.col("start") + 100, pl.col("end") + 100, k=2))
    assert mask.to_series().to_list() == frame.select(expr(2)).to_series().to_list()


def test_date_days_and_datetime_physical_units():
    frame = pl.DataFrame(
        {"start": [date(2026, 1, 1), date(2026, 1, 5)], "end": [date(2026, 1, 5), date(2026, 1, 7)]}
    )
    for k, coverage in [(1, 4), (2, 6)]:
        mask = frame.select(expr(k)).to_series()
        verify(frame, mask, k)
        assert (
            objective(
                list(zip(frame["start"].to_physical(), frame["end"].to_physical())),
                [i for i, b in enumerate(mask) if b],
            )[0]
            == coverage
        )
    # Across the Helsinki DST spring jump, elapsed physical time is one hour.
    for unit, factor in [("ms", 1_000), ("us", 1_000_000), ("ns", 1_000_000_000)]:
        frame = pl.DataFrame(
            {
                "start": [datetime(2026, 3, 29, 0, tzinfo=UTC)],
                "end": [datetime(2026, 3, 29, 1, tzinfo=UTC)],
            }
        ).select(pl.all().dt.convert_time_zone("Europe/Helsinki").dt.cast_time_unit(unit))
        assert frame["start"].dt.hour()[0] == 2 and frame["end"].dt.hour()[0] == 4
        assert frame["end"].to_physical()[0] - frame["start"].to_physical()[0] == 3600 * factor
        verify(frame, frame.select(expr(1)).to_series(), 1)


@pytest.mark.parametrize(
    "dtype,low,high", [(pl.Int64, -(2**63), 2**63 - 1), (pl.UInt64, 0, 2**64 - 1)]
)
def test_extreme_arithmetic(dtype, low, high):
    frame = pl.DataFrame(
        {"start": [low, 1, low], "end": [1, high, low]},
        schema_overrides={"start": dtype, "end": dtype},
    )
    for k in [0, 1, 2, 10]:
        verify(frame, frame.select(expr(k)).to_series(), k)


@pytest.mark.parametrize("k", [True, False, 1.5, "2", None])
def test_rejects_non_integer_budget(k):
    with pytest.raises(TypeError, match="k"):
        expr(k)


@pytest.mark.parametrize("k", [-1, 2**64, 2**100])
def test_rejects_out_of_range_budget(k):
    with pytest.raises(ValueError, match="k"):
        expr(k)


def test_random_native_against_brute_force():
    random = Random(20260927)
    for _ in range(100):
        rows = [
            sorted([random.randint(-8, 12), random.randint(-8, 12)])
            for _ in range(random.randint(0, 10))
        ]
        frame = pl.DataFrame(rows, schema={"start": pl.Int64, "end": pl.Int64}, orient="row")
        k = random.randrange(12)
        verify(frame, frame.select(expr(k)).to_series(), k)
