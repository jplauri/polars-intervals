"""The eager profile API, exercised through the installed native extension."""

from datetime import UTC, date, datetime
from random import Random

import polars as pl
import polars_intervals as pi
import pytest

from .dtypes import ENDPOINT_DTYPES, INTEGER_DTYPES


def frames(jobs, profile, endpoint=pl.Int64, weight=pl.Int64, capacity=pl.Int64):
    return (
        pl.DataFrame(
            jobs, schema={"start": endpoint, "end": endpoint, "weight": weight}, orient="row"
        ),
        pl.DataFrame(
            profile, schema={"start": endpoint, "end": endpoint, "capacity": capacity}, orient="row"
        ),
    )


def objective(jobs, mask):
    return sum(w for w, chosen in zip(jobs["weight"], mask, strict=True) if chosen)


def assert_optimal(jobs, profile, mask):
    """Independent exhaustive oracle: inspect every atomic segment for every subset."""
    s, e = (jobs[name].to_physical().to_list() for name in ("start", "end"))
    ps, pe = (profile[name].to_physical().to_list() for name in ("start", "end"))
    c, w = profile["capacity"].to_list(), jobs["weight"].to_list()
    points = sorted(set(s + e + ps + pe))

    def feasible(indices):
        for t in points[:-1]:
            available = next((cap for a, b, cap in zip(ps, pe, c, strict=True) if a <= t < b), 0)
            if sum(s[i] <= t < e[i] for i in indices) > available:
                return False
        return True

    optimum = 0
    for bits in range(1 << len(jobs)):
        chosen = [i for i in range(len(jobs)) if bits & (1 << i)]
        if feasible(chosen):
            optimum = max(optimum, sum(w[i] for i in chosen))
    assert mask.name == "selected"
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(jobs)
    assert feasible([i for i, chosen in enumerate(mask) if chosen])
    assert objective(jobs, mask) == optimum
    assert all(w[i] > 0 for i, chosen in enumerate(mask) if chosen)
    assert all(mask[i] for i in range(len(jobs)) if s[i] == e[i] and w[i] > 0)


CASES = [
    pytest.param([], [], id="empty-jobs-and-profile"),
    pytest.param([], [(0, 10, 2)], id="empty-jobs"),
    pytest.param(
        [(0, 4, 10), (2, 2, 3), (2, 2, 0), (2, 2, -1)], [], id="empty-profile-positive-empties"
    ),
    pytest.param(
        [(0, 12, 100), (0, 5, 10), (7, 12, 20)],
        [(0, 5, 2), (5, 7, 0), (7, 12, 2)],
        id="zero-capacity-gap",
    ),
    pytest.param(
        [(0, 12, 30), (0, 12, 29), (0, 4, 20), (8, 12, 21)],
        [(0, 4, 3), (4, 8, 1), (8, 12, 3)],
        id="capacity-bottleneck",
    ),
    pytest.param(
        [(0, 10, 30), (0, 10, 29), (0, 10, 28)],
        [(0, 4, 10**9), (4, 5, 1), (5, 10, 10**9)],
        id="huge-capacity-does-not-rescue-bottleneck",
    ),
    pytest.param(
        [(0, 10, 9), (0, 5, 10), (5, 10, 6), (5, 10, 7), (5, 10, 8)],
        [(0, 5, 1), (5, 10, 3)],
        id="capacity-increase",
    ),
    pytest.param(
        [(0, 10, 9), (0, 5, 10), (0, 5, 6), (5, 10, 7)],
        [(0, 5, 3), (5, 10, 1)],
        id="capacity-decrease",
    ),
    pytest.param(
        [(0, 10, 100), (0, 5, 60), (5, 10, 60)],
        [(0, 4, 2), (4, 6, 1), (6, 10, 2)],
        id="greedy-weight-blocks-better-short-jobs",
    ),
    pytest.param(
        [(0, 10, w) for w in (9, 7, 5, 3, 1)],
        [(0, 3, 4), (3, 7, 2), (7, 10, 3)],
        id="varying-capacity-clique-top-k",
    ),
    pytest.param(
        [(-1, 2, 100), (0, 2, 10), (8, 12, 90), (11, 12, 20)],
        [(0, 10, 8)],
        id="outside-profile-capacity-zero",
    ),
    pytest.param(
        [(0, 2, 10), (2, 5, 9), (5, 8, 8), (8, 8, 7), (0, 8, -1)],
        [(0, 5, 1), (5, 8, 2)],
        id="all-positive-feasible",
    ),
]


@pytest.mark.parametrize("rows,segments", CASES)
def test_named_regressions(rows, segments):
    jobs, profile = frames(rows, segments)
    mask = pi.max_weight_with_capacity_profile(jobs, profile)
    assert_optimal(jobs, profile, mask)
    assert mask.equals(pi.max_weight_with_capacity_profile(jobs, profile))


@pytest.mark.parametrize("endpoint", ENDPOINT_DTYPES, ids=str)
def test_endpoint_dtypes_chunks_slices_and_order(endpoint):
    jobs, profile = frames(
        [(0, 10, 25), (0, 4, 10), (4, 7, 10), (7, 10, 10), (2, 2, 8)],
        [(7, 10, 2), (0, 4, 3), (4, 7, 1)],
        endpoint,
    )
    jobs = jobs[[3, 0, 4, 1, 2]]
    jobs = pl.concat([jobs.head(2), jobs.slice(2)], rechunk=False)
    profile = pl.concat([profile.head(1), profile.slice(1)], rechunk=False)
    assert all(column.n_chunks() == 2 for column in jobs)
    assert all(column.n_chunks() == 2 for column in profile)
    for subset in (jobs, jobs.slice(1, 3), jobs.head(0)):
        mask = pi.max_weight_with_capacity_profile(subset, profile)
        assert_optimal(subset, profile, mask)
        assert mask.equals(pi.max_weight_with_capacity_profile(subset.rechunk(), profile.rechunk()))


@pytest.mark.parametrize("weight", INTEGER_DTYPES, ids=str)
@pytest.mark.parametrize("capacity", INTEGER_DTYPES, ids=str)
def test_weight_capacity_integer_dtypes(weight, capacity):
    jobs, profile = frames(
        [(0, 10, 12), (0, 5, 8), (5, 10, 7), (2, 2, 3)],
        [(0, 5, 2), (5, 10, 1)],
        weight=weight,
        capacity=capacity,
    )
    assert_optimal(jobs, profile, pi.max_weight_with_capacity_profile(jobs, profile))


@pytest.mark.parametrize("k", [0, 1, 2, 3, 8, 64, 2**64 - 1])
def test_constant_profile_equivalence_and_clamping(k):
    jobs, profile = frames(
        [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10), (1, 9, 25), (2, 2, 8)],
        [(0, 10, k)],
        capacity=pl.UInt64,
    )
    mask = pi.max_weight_with_capacity_profile(jobs, profile)
    scalar = jobs.select(
        pi.max_weight_with_capacity("start", "end", weight="weight", capacity=k)
    ).to_series()
    assert objective(jobs, mask) == objective(jobs, scalar)
    if k == 1:
        one = jobs.select(
            pi.max_weight_non_overlapping("start", "end", weight="weight")
        ).to_series()
        assert objective(jobs, mask) == objective(jobs, one)
    clamped = profile.with_columns(pl.col("capacity").clip(upper_bound=5))
    assert objective(jobs, mask) == objective(
        jobs, pi.max_weight_with_capacity_profile(jobs, clamped)
    )
    assert_optimal(jobs, profile, mask)


def test_gaps_splitting_coalescing_empty_rows_and_profile_permutation():
    jobs, _ = frames([(0, 10, 50), (0, 5, 9), (7, 10, 8), (0, 5, 7), (5, 5, 6)], [])
    profiles = [
        [(0, 5, 2), (7, 10, 1)],
        [(0, 5, 2), (5, 7, 0), (7, 10, 1)],
        [(0, 2, 2), (2, 5, 2), (7, 10, 1)],
        [(7, 10, 1), (0, 5, 2), (5, 5, 100)],
    ]
    masks = [pi.max_weight_with_capacity_profile(jobs, frames([], rows)[1]) for rows in profiles]
    assert all(mask.equals(masks[0]) for mask in masks)


def test_custom_columns_and_original_row_alignment():
    jobs, profile = frames([(0, 10, 11), (0, 5, 8), (5, 10, 7)], [(0, 4, 2), (4, 6, 1), (6, 10, 2)])
    jobs = jobs[[2, 0, 1]].rename({"start": "s", "end": "e", "weight": "w"})
    profile = profile.rename({"start": "a", "end": "b", "capacity": "c"})
    mask = pi.max_weight_with_capacity_profile(
        jobs,
        profile,
        start="s",
        end="e",
        weight="w",
        profile_start="a",
        profile_end="b",
        capacity="c",
    )
    assert mask.to_list() == [True, False, True]


@pytest.mark.parametrize("profile_args", [(), (None,)])
def test_single_frame_profile_preserves_original_row_alignment(profile_args):
    jobs, profile = frames([(5, 10, 7), (0, 10, 11), (0, 5, 8)], [(0, 4, 2), (4, 6, 1), (6, 10, 2)])
    combined = jobs.hstack(profile.rename({"start": "cap_start", "end": "cap_end"}))
    mask = pi.max_weight_with_capacity_profile(
        combined, *profile_args, profile_start="cap_start", profile_end="cap_end"
    )
    assert mask.equals(pi.max_weight_with_capacity_profile(jobs, profile))
    assert combined.with_columns(mask)["selected"].to_list() == [True, False, True]
    assert_optimal(jobs, profile, mask)


def test_single_frame_default_columns_and_explicit_empty_profile():
    jobs, profile = frames([(0, 5, 8), (5, 10, 9)], [(0, 5, 1), (5, 10, 0)])
    combined = jobs.with_columns(profile["capacity"])
    assert pi.max_weight_with_capacity_profile(combined).to_list() == [True, False]
    assert pi.max_weight_with_capacity_profile(combined, profile.clear()).to_list() == [
        False,
        False,
    ]


def test_variable_profile_across_full_unsigned_endpoint_range():
    maximum = 2**64 - 1
    jobs, profile = frames(
        [(0, maximum, maximum), (maximum - 1, maximum, maximum - 1), (0, 2**63, maximum - 2)],
        [(0, 2**63, 2), (2**63, maximum, 1)],
        endpoint=pl.UInt64,
        weight=pl.UInt64,
    )
    mask = pi.max_weight_with_capacity_profile(jobs, profile)
    assert mask.to_list() == [True, False, True]
    assert_optimal(jobs, profile, mask)


@pytest.mark.parametrize(
    "role,column",
    [
        ("jobs", "start"),
        ("jobs", "end"),
        ("jobs", "weight"),
        ("profile", "start"),
        ("profile", "end"),
        ("profile", "capacity"),
    ],
)
def test_nulls_rejected(role, column):
    jobs, profile = frames([(0, 1, 1)], [(0, 1, 1)])
    inputs = {"jobs": jobs, "profile": profile}
    inputs[role] = inputs[role].with_columns(pl.lit(None, dtype=pl.Int64).alias(column))
    with pytest.raises(pl.exceptions.PolarsError, match="null"):
        pi.max_weight_with_capacity_profile(inputs["jobs"], inputs["profile"])


@pytest.mark.parametrize("empty_jobs", [False, True])
@pytest.mark.parametrize(
    "rows,message",
    [
        ([(0, 7, 2), (5, 10, 3)], "overlap"),
        ([(0, 7, 0), (5, 10, 0)], "overlap"),
        ([(2, 1, 0)], "profile.*index 0"),
        ([(0, 1, -1)], "capacity.*negative"),
        ([(5, 5, -1)], "capacity.*negative"),
    ],
)
def test_invalid_profiles_are_validated_before_fast_paths(empty_jobs, rows, message):
    jobs, profile = frames([] if empty_jobs else [(0, 1, 1)], rows)
    with pytest.raises(pl.exceptions.PolarsError, match=message):
        pi.max_weight_with_capacity_profile(jobs, profile)


@pytest.mark.parametrize("profile", [[], [(0, 10, 0)], [(0, 10, 10)]])
def test_reversed_jobs_even_when_nonpositive_or_profile_zero(profile):
    jobs, profile = frames([(0, 1, 3), (3, 2, -1)], profile)
    with pytest.raises(pl.exceptions.PolarsError, match="index 1"):
        pi.max_weight_with_capacity_profile(jobs, profile)


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float32,
        pl.Float64,
        pl.Int128,
        pl.Decimal(20, 2),
        pl.Boolean,
        pl.String,
        pl.Null,
        pl.Date,
        pl.Datetime("us"),
        pl.Time,
        pl.Duration("us"),
    ],
    ids=str,
)
@pytest.mark.parametrize("role", ["weight", "capacity"])
@pytest.mark.parametrize("empty", [False, True])
def test_unsupported_weight_capacity_no_casts(dtype, role, empty):
    jobs, profile = frames([] if empty else [(0, 1, 1)], [] if empty else [(0, 1, 1)])
    replacement = pl.Series(role, [] if empty else [None], dtype=dtype)
    if role == "weight":
        jobs = jobs.with_columns(replacement)
    else:
        profile = profile.with_columns(replacement)
    with pytest.raises(pl.exceptions.PolarsError, match=f"integer {role} dtype"):
        pi.max_weight_with_capacity_profile(jobs, profile)


@pytest.mark.parametrize(
    "job_dtype,profile_dtype",
    [
        (pl.Int32, pl.Int64),
        (pl.Int64, pl.UInt64),
        (pl.Date, pl.Int32),
        (pl.Date, pl.Datetime("ms")),
        (pl.Datetime("us"), pl.Datetime("ns")),
        (pl.Datetime("ns", "UTC"), pl.Datetime("ns", "Europe/Helsinki")),
        (pl.Datetime("us"), pl.Datetime("us", "UTC")),
    ],
)
@pytest.mark.parametrize("column", ["start", "end", "both"])
def test_exact_logical_endpoint_dtype_compatibility(job_dtype, profile_dtype, column):
    jobs, profile = frames([(0, 1, 1)], [(0, 1, 1)], endpoint=job_dtype)
    columns = ["start", "end"] if column == "both" else [column]
    profile = profile.with_columns(pl.col(columns).cast(profile_dtype))
    with pytest.raises(pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"):
        pi.max_weight_with_capacity_profile(jobs, profile)


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Float64,
        pl.Int128,
        pl.Decimal(20, 2),
        pl.Boolean,
        pl.String,
        pl.Null,
        pl.Time,
        pl.Duration("us"),
    ],
    ids=str,
)
def test_unsupported_endpoints_on_empty_inputs(dtype):
    jobs, profile = frames([], [], endpoint=dtype)
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype, Date, or Datetime"):
        pi.max_weight_with_capacity_profile(jobs, profile)


@pytest.mark.parametrize(
    "dtype",
    [
        pl.Categorical,
        pl.Enum(["a"]),
        pl.Object,
        pl.List(pl.Int64),
        pl.Array(pl.Int64, 2),
        pl.Struct({"x": pl.Int64}),
    ],
    ids=str,
)
@pytest.mark.parametrize(
    "role,column",
    [
        ("jobs", "start"),
        ("jobs", "end"),
        ("jobs", "weight"),
        ("profile", "start"),
        ("profile", "end"),
        ("profile", "capacity"),
    ],
)
@pytest.mark.parametrize("empty", [False, True])
def test_optional_polars_dtypes_rejected_before_native_import(dtype, role, column, empty):
    # These Arrow types can require optional native Polars features. Reject them
    # without enabling those dependencies merely to produce an unsupported error.
    jobs, profile = frames([] if empty else [(0, 1, 1)], [] if empty else [(0, 1, 1)])
    inputs = {"jobs": jobs, "profile": profile}
    inputs[role] = inputs[role].with_columns(
        pl.Series(column, [] if empty else [None], dtype=dtype)
    )
    with pytest.raises(pl.exceptions.InvalidOperationError, match="requires an 8-"):
        pi.max_weight_with_capacity_profile(inputs["jobs"], inputs["profile"])


def test_eager_only_and_missing_columns():
    jobs, profile = frames([(0, 1, 1)], [(0, 1, 1)])
    for inputs in (
        (jobs.lazy(), profile),
        (jobs, profile.lazy()),
        ([], profile),
        (jobs.lazy(),),
        ([],),
    ):
        with pytest.raises(TypeError, match="eager Polars DataFrames"):
            pi.max_weight_with_capacity_profile(*inputs)
    with pytest.raises(TypeError, match="column names"):
        pi.max_weight_with_capacity_profile(jobs, profile, start=pl.col("start"))
    with pytest.raises(pl.exceptions.ColumnNotFoundError):
        pi.max_weight_with_capacity_profile(jobs, profile, profile_start="missing")
    with pytest.raises(pl.exceptions.ColumnNotFoundError):
        pi.max_weight_with_capacity_profile(jobs)


@pytest.mark.parametrize("dtype", [pl.Int64, pl.UInt64])
def test_large_weights_are_exact(dtype):
    maximum = 2**64 - 1 if dtype == pl.UInt64 else 2**63 - 1
    jobs, profile = frames(
        [(0, 10, maximum - 2), (0, 10, maximum), (0, 10, maximum - 1), (2, 2, maximum)],
        [(0, 5, 3), (5, 10, 2)],
        weight=dtype,
    )
    mask = pi.max_weight_with_capacity_profile(jobs, profile)
    assert mask.to_list() == [False, True, True, True]
    assert_optimal(jobs, profile, mask)


def test_real_dates_and_timezone_transition():
    for dtype, values in [
        (pl.Date, [date(2026, 1, d) for d in (1, 2, 3)]),
        (
            pl.Datetime("us", "Europe/Helsinki"),
            [datetime(2026, 10, 25, h, m, tzinfo=UTC) for h, m in ((0, 30), (1, 0), (1, 30))],
        ),
    ]:
        a, b, c = values
        jobs, profile = frames(
            [(a, c, 20), (a, b, 11), (b, c, 12), (b, b, 5)], [(a, b, 2), (b, c, 1)], endpoint=dtype
        )
        assert_optimal(jobs, profile, pi.max_weight_with_capacity_profile(jobs, profile))


def test_generated_profiles_against_independent_exhaustive_oracle():
    rng = Random(20260927)
    for _ in range(100):
        rows = []
        for _ in range(rng.randrange(9)):
            a, b = sorted((rng.randrange(-4, 7), rng.randrange(-4, 7)))
            rows.append((a, b, rng.randrange(-10, 21)))
        segments = [(t, t + 1, c) for t in range(-3, 6) if (c := rng.randrange(5))]
        rng.shuffle(rows)
        rng.shuffle(segments)
        jobs, profile = frames(rows, segments)
        assert_optimal(jobs, profile, pi.max_weight_with_capacity_profile(jobs, profile))
