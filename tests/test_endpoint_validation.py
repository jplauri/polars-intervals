import polars as pl
import polars_intervals as pi
import pytest

pytestmark = pytest.mark.parametrize("algorithm", [pi.containment_count, pi.nesting_depth])


@pytest.mark.parametrize("column", ["start", "end", "both"])
@pytest.mark.parametrize("dtype", [pl.Int64, pl.Date, pl.Datetime("us", "UTC")])
def test_null_endpoints_are_rejected(algorithm, column, dtype):
    frame = pl.DataFrame({"start": [0, 1], "end": [2, 3]}).cast(dtype)
    names = ["start", "end"] if column == "both" else [column]
    frame = frame.with_columns(pl.lit(None, dtype=dtype).alias(name) for name in names)
    with pytest.raises(pl.exceptions.ComputeError, match="null endpoints"):
        frame.select(algorithm("start", "end"))


@pytest.mark.parametrize(
    "start_dtype,end_dtype",
    [
        (pl.Int32, pl.Int64),
        (pl.UInt64, pl.Int64),
        (pl.Date, pl.Int32),
        (pl.Datetime("ms"), pl.Datetime("us")),
        (pl.Datetime("us"), pl.Datetime("ns")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us")),
        (pl.Datetime("us", "UTC"), pl.Datetime("us", "Europe/Helsinki")),
    ],
)
def test_logical_dtypes_must_match_exactly(algorithm, start_dtype, end_dtype):
    frame = pl.DataFrame(
        {"start": pl.Series([0], dtype=start_dtype), "end": pl.Series([1], dtype=end_dtype)}
    )
    with pytest.raises(pl.exceptions.PolarsError, match="matching.*dtypes"):
        frame.select(algorithm("start", "end"))


@pytest.mark.parametrize(
    "dtype", [pl.Float64, pl.Boolean, pl.String, pl.Duration("us"), pl.Time, pl.Int128]
)
def test_unsupported_dtypes_even_when_empty(algorithm, dtype):
    frame = pl.DataFrame(schema={"start": dtype, "end": dtype})
    with pytest.raises(pl.exceptions.PolarsError, match="integer dtype"):
        frame.select(algorithm("start", "end"))


def test_invalid_interval_reports_first_original_index(algorithm):
    frame = pl.DataFrame({"start": [0, 3, 4], "end": [0, 2, 1]})
    with pytest.raises(pl.exceptions.ComputeError, match="index 1"):
        frame.select(algorithm("start", "end"))


@pytest.mark.parametrize("start", [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)])
def test_lengths_must_match_without_broadcasting(algorithm, start):
    frame = pl.DataFrame({"start": [0, 1], "end": [3, 4]})
    with pytest.raises(pl.exceptions.PolarsError, match="equal lengths"):
        frame.select(algorithm(start, "end"))
