"""Endpoint validation shared by every two-column interval expression."""

import polars as pl
import polars_intervals as pi
import pytest

from .dtypes import ENDPOINT_DTYPES

# A non-null integer column of the frame's length, for weight, cost, and lane arguments.
ROWS = pl.int_range(pl.len())
# Fast-path settings (k=0, capacity=0, max_work=0) must validate like the rest.
ALGORITHMS = {
    "overlap_count": pi.overlap_count,
    "containment_count": pi.containment_count,
    "nesting_depth": pi.nesting_depth,
    "assign_lanes": pi.assign_lanes,
    "balanced_construct": lambda s, e: pi.assign_balanced_lanes(s, e, max_work=0),
    "balanced_repair": lambda s, e: pi.assign_balanced_lanes(s, e, initial_lanes=ROWS),
    "max_k_coverage_k0": lambda s, e: pi.max_k_coverage(s, e, k=0),
    "max_k_coverage_k2": lambda s, e: pi.max_k_coverage(s, e, k=2),
    "minimum_stabbing_points": pi.minimum_stabbing_points,
    "domination_units": pi.minimum_cost_dominating_set,
    "domination_costs": lambda s, e: pi.minimum_cost_dominating_set(s, e, cost=ROWS),
    "max_weight_non_overlapping": lambda s, e: pi.max_weight_non_overlapping(s, e, weight=ROWS),
    "capacity_0": lambda s, e: pi.max_weight_with_capacity(s, e, weight=ROWS, capacity=0),
    "capacity_2": lambda s, e: pi.max_weight_with_capacity(s, e, weight=ROWS, capacity=2),
    "coverage_profile": pi.coverage_profile,
}
pytestmark = pytest.mark.parametrize("algorithm", ALGORITHMS.values(), ids=ALGORITHMS.keys())


def evaluate(frame, algorithm, start="start", end="end"):
    if algorithm is pi.coverage_profile:
        return algorithm(frame, start=start, end=end)
    return frame.select(algorithm(start, end))


@pytest.mark.parametrize("dtype", ENDPOINT_DTYPES, ids=str)
def test_reversed_and_null_endpoints(algorithm, dtype):
    frame = pl.DataFrame({"start": [0, 3, 4], "end": [1, 2, 1]}).cast(dtype)
    with pytest.raises(pl.exceptions.ComputeError, match="index 1"):
        evaluate(frame, algorithm)
    for columns in (["start"], ["end"], ["start", "end"]):
        nulls = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in columns)
        with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
            evaluate(nulls, algorithm)


@pytest.mark.parametrize(
    "start_dtype,end_dtype",
    [
        (pl.Int32, pl.Int64),
        (pl.Int32, pl.UInt32),
        (pl.Date, pl.Int32),
        (pl.Date, pl.Datetime("ms")),
        (pl.Datetime("ms"), pl.Int64),
        (pl.Datetime("ms"), pl.Datetime("us")),
        (pl.Datetime("us"), pl.Datetime("ns")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us")),
        (pl.Datetime("ns", "UTC"), pl.Datetime("ns", "Europe/Helsinki")),
    ],
)
def test_logical_dtypes_must_match_exactly(algorithm, start_dtype, end_dtype):
    for values in ([0], []):
        frame = pl.DataFrame(
            {
                "start": pl.Series(values, dtype=start_dtype),
                "end": pl.Series(values, dtype=end_dtype),
            }
        )
        for start, end in (("start", "end"), ("end", "start")):
            with pytest.raises(
                pl.exceptions.PolarsError, match="matching integer, Date, or Datetime"
            ):
                evaluate(frame, algorithm, start, end)


CHECKED = [pl.Float32, pl.Float64, pl.Boolean, pl.String, pl.Null, pl.Time, pl.Duration("us")]
CHECKED += [pl.Int128, pl.Decimal(20, 0)]
# Dtypes whose optional Polars features are disabled fail on import instead.
UNIMPORTED = [pl.UInt128, pl.Categorical, pl.Enum(["a"]), pl.Object, pl.List(pl.Int64)]
UNIMPORTED += [pl.Array(pl.Int64, 2), pl.Struct({"x": pl.Int64})]


@pytest.mark.parametrize("dtype", CHECKED + UNIMPORTED, ids=str)
def test_unsupported_dtypes_even_when_empty(algorithm, dtype):
    message = "integer dtype, Date, or Datetime" if dtype in CHECKED else None
    for values in ([None], []):
        frame = pl.DataFrame({name: pl.Series(values, dtype=dtype) for name in ("start", "end")})
        with pytest.raises(pl.exceptions.PolarsError, match=message):
            evaluate(frame, algorithm)


@pytest.mark.parametrize("argument", ["start", "end"])
@pytest.mark.parametrize("scalar", [False, True])
def test_lengths_must_match_without_broadcasting(algorithm, argument, scalar):
    frame = pl.DataFrame({"start": [0, 1], "end": [3, 4]})
    args = {"start": pl.col("start"), "end": pl.col("end")}
    args[argument] = pl.lit(1, dtype=pl.Int64) if scalar else args[argument].head(1)
    if algorithm is pi.coverage_profile:
        # DataFrames enforce column lengths; direct Series mismatch coverage is
        # in the Rust integration suite. Expressions are rejected explicitly.
        with pytest.raises(TypeError, match="strings"):
            evaluate(frame, algorithm, args["start"], args["end"])
        return
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(algorithm(args["start"], args["end"]))
