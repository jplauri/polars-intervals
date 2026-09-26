//! Interval algorithms for Rust Polars, backed by `intervals-core`.
//!
//! [`overlap_count`], [`assign_lanes`] and [`max_weight_non_overlapping`] adapt
//! integer, Date, and Datetime [`Series`] to the core and Python expressions.

use polars::prelude::*;
use std::borrow::Cow;

#[pyo3::pymodule]
mod _internal {}

// The output_type_func form also catches failures while importing field dtypes.
// The constant output_type form can abort at the FFI boundary for a dtype whose
// optional Polars feature is disabled, before our validation runs.
fn count_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, Algorithm::OverlapCount)?;
    Ok(Field::new(start.name().clone(), DataType::UInt64))
}

fn lanes_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, Algorithm::AssignLanes)?;
    Ok(Field::new(start.name().clone(), DataType::UInt32))
}

fn input_pair<'a, T>(inputs: &'a [T], algorithm: Algorithm<'_>) -> PolarsResult<&'a [T; 2]> {
    inputs.try_into().map_err(|_| {
        polars_err!(InvalidOperation: "{} requires exactly two inputs, got {}",
            algorithm.name(), inputs.len())
    })
}

#[pyo3_polars::derive::polars_expr(output_type_func = count_output)]
fn overlap_count_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, Algorithm::OverlapCount)?;
    overlap_count(starts, ends)
}

#[pyo3_polars::derive::polars_expr(output_type_func = lanes_output)]
fn assign_lanes_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, Algorithm::AssignLanes)?;
    assign_lanes(starts, ends)
}

fn weighted_inputs<T>(inputs: &[T]) -> PolarsResult<&[T; 3]> {
    inputs.try_into().map_err(|_| {
        polars_err!(InvalidOperation: "max_weight_non_overlapping requires exactly three inputs, got {}", inputs.len())
    })
}

fn weighted_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _, weight] = weighted_inputs(inputs)?;
    validate_weight_dtype(weight.dtype())?;
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

fn validate_weight_dtype(dtype: &DataType) -> PolarsResult<()> {
    polars_ensure!(matches!(dtype,
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 |
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64),
        InvalidOperation:
        "max_weight_non_overlapping requires an 8-, 16-, 32-, or 64-bit integer weight dtype, got {}", dtype);
    Ok(())
}

#[pyo3_polars::derive::polars_expr(output_type_func = weighted_output)]
fn max_weight_non_overlapping_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends, weights] = weighted_inputs(inputs)?;
    max_weight_non_overlapping(starts, ends, weights)
}

/// Counts other overlapping intervals in the supplied start and end columns.
///
/// Both columns must have the same dtype: `Int8`, `Int16`, `Int32`, `Int64`,
/// `UInt8`, `UInt16`, `UInt32`, `UInt64`, `Date`, or `Datetime`. Datetime units
/// (ms, us, ns) and timezone metadata must match exactly. No dtype coercion is
/// performed. Dates use physical integer days and datetimes use physical integer
/// timestamps, preserving endpoint values without timezone arithmetic.
/// Nulls are rejected in either column, even when both endpoints are null.
///
/// Intervals use half-open `[start, end)` semantics. Two non-empty intervals
/// overlap when `a.start < b.end && b.start < a.end`. Touching endpoints do not
/// overlap. Empty intervals (`start == end`) overlap nothing. Each row excludes
/// itself, while duplicate non-empty intervals count each other.
///
/// Returns a non-null `UInt64` Series named `overlap_count`, with one count per
/// input row in its original order. All chunks belong to the same collection.
/// The core algorithm takes `O(n log n)` time and `O(n)` additional space.
///
/// # Errors
///
/// Returns a [`PolarsError`] for unequal lengths (without broadcasting),
/// mismatched or unsupported dtypes, null endpoints, or any `start > end`.
/// Invalid intervals report their zero-based index in the original input.
///
/// # Examples
///
/// ```
/// use polars::prelude::*;
/// use polars_intervals::overlap_count;
///
/// let starts = Series::new("start".into(), [1i64, 3, 2, 2]);
/// let ends = Series::new("end".into(), [3i64, 5, 4, 2]);
/// let counts = overlap_count(&starts, &ends)?;
/// assert_eq!(counts.u64()?.into_no_null_iter().collect::<Vec<_>>(), [1, 1, 2, 0]);
/// # Ok::<(), PolarsError>(())
/// ```
pub fn overlap_count(starts: &Series, ends: &Series) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::OverlapCount)
}

/// Assign intervals to the minimum number of non-overlapping lanes.
///
/// Returns a non-null `UInt32` Series named `assign_lanes`, in original row
/// order. Lane IDs are contiguous from zero and deterministic for identical
/// input; no particular optimal coloring is promised across releases.
/// The minimum lane count equals maximum concurrency of non-empty intervals.
/// Empty intervals consume no capacity and receive lane zero. An all-empty,
/// nonempty collection uses one lane; empty input returns empty output.
///
/// Uses the same matching integer, Date and Datetime dtypes, half-open semantics,
/// null rejection, physical representations, and chunk handling as [`overlap_count`].
/// Takes `O(n log n)` time and `O(n)` additional space.
///
/// # Errors
///
/// Returns a [`PolarsError`] for unequal lengths, mismatched or unsupported
/// logical dtypes, null endpoints, reversed intervals (with original row index),
/// or lane IDs exceeding `u32::MAX`. There is no coercion or broadcasting.
///
/// # Examples
///
/// ```
/// use polars::prelude::*;
/// use polars_intervals::assign_lanes;
/// let starts = Series::new("start".into(), [0i64, 1, 2]);
/// let ends = Series::new("end".into(), [2i64, 3, 4]);
/// let lanes = assign_lanes(&starts, &ends)?;
/// assert_eq!(lanes.n_unique()?, 2);
/// # Ok::<(), PolarsError>(())
/// ```
pub fn assign_lanes(starts: &Series, ends: &Series) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::AssignLanes)
}

/// Select a globally maximum-weight subset of mutually non-overlapping intervals.
///
/// Returns a non-null Boolean Series named `max_weight_non_overlapping` in
/// original row order. Endpoints follow the integer, Date and Datetime rules of
/// [`overlap_count`], including matching units/timezones and half-open semantics.
/// Weights must be non-null signed/unsigned integers of at most 64 bits. They are
/// widened losslessly to `i128` for checked accumulation, never through floats.
///
/// The empty subset is allowed. Every positive empty interval is selected;
/// nonpositive rows are omitted. Identical inputs give deterministic results,
/// but no particular optimum on ties is promised across releases. All chunks
/// form one instance. Takes `O(n log n)` time and `O(n)` additional space.
///
/// # Errors
///
/// Rejects unequal lengths, nulls, unsupported weight or endpoint dtypes,
/// mismatched logical endpoint dtypes, reversed intervals (original row index),
/// and objectives exceeding `i128`. No coercion or broadcasting is performed.
///
/// # Examples
///
/// ```
/// use polars::prelude::*;
/// use polars_intervals::max_weight_non_overlapping;
/// let s = Series::new("s".into(), [0i64, 0, 4, 7]);
/// let e = Series::new("e".into(), [10i64, 4, 7, 10]);
/// let w = Series::new("w".into(), [15u64, 10, 10, 10]);
/// let mask = max_weight_non_overlapping(&s, &e, &w)?;
/// assert_eq!(mask.bool()?.no_null_iter().collect::<Vec<_>>(), [false, true, true, true]);
/// # Ok::<(), PolarsError>(())
/// ```
pub fn max_weight_non_overlapping(
    starts: &Series,
    ends: &Series,
    weights: &Series,
) -> PolarsResult<Series> {
    validate_weight_dtype(weights.dtype())?;
    polars_ensure!(starts.len() == weights.len(), ShapeMismatch:
        "max_weight_non_overlapping requires equal lengths, got {} intervals and {} weights",
        starts.len(), weights.len());
    polars_ensure!(weights.null_count() == 0, ComputeError:
        "max_weight_non_overlapping does not support null weights");
    // Normalize only the accumulator representation, avoiding 8 x 8 endpoint /
    // weight monomorphizations. No Polars cast or loss of integer precision.
    macro_rules! widen {
        ($accessor:ident) => {
            weights
                .$accessor()?
                .into_no_null_iter()
                .map(i128::from)
                .collect::<Vec<_>>()
        };
    }
    let weights = match weights.dtype() {
        DataType::Int8 => widen!(i8),
        DataType::Int16 => widen!(i16),
        DataType::Int32 => widen!(i32),
        DataType::Int64 => widen!(i64),
        DataType::UInt8 => widen!(u8),
        DataType::UInt16 => widen!(u16),
        DataType::UInt32 => widen!(u32),
        DataType::UInt64 => widen!(u64),
        _ => unreachable!("weight dtype was validated above"),
    };
    evaluate(starts, ends, Algorithm::MaxWeight(&weights))
}

#[derive(Clone, Copy)]
enum Algorithm<'a> {
    OverlapCount,
    AssignLanes,
    MaxWeight(&'a [i128]),
}

impl Algorithm<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::OverlapCount => "overlap_count",
            Self::AssignLanes => "assign_lanes",
            Self::MaxWeight(_) => "max_weight_non_overlapping",
        }
    }
}

fn evaluate(starts: &Series, ends: &Series, algorithm: Algorithm<'_>) -> PolarsResult<Series> {
    let name = algorithm.name();
    polars_ensure!(
        starts.len() == ends.len(),
        ShapeMismatch: "{} requires equal lengths, got {} starts and {} ends", name,
        starts.len(), ends.len()
    );
    polars_ensure!(
        starts.dtype() == ends.dtype(),
        InvalidOperation: "{} requires matching integer, Date, or Datetime dtypes (including Datetime time unit and timezone), got {} and {}", name,
        starts.dtype(), ends.dtype()
    );
    match starts.dtype() {
        DataType::Int8 => evaluate_typed(starts.i8()?, ends.i8()?, algorithm),
        DataType::Int16 => evaluate_typed(starts.i16()?, ends.i16()?, algorithm),
        DataType::Int32 => evaluate_typed(starts.i32()?, ends.i32()?, algorithm),
        DataType::Int64 => evaluate_typed(starts.i64()?, ends.i64()?, algorithm),
        DataType::UInt8 => evaluate_typed(starts.u8()?, ends.u8()?, algorithm),
        DataType::UInt16 => evaluate_typed(starts.u16()?, ends.u16()?, algorithm),
        DataType::UInt32 => evaluate_typed(starts.u32()?, ends.u32()?, algorithm),
        DataType::UInt64 => evaluate_typed(starts.u64()?, ends.u64()?, algorithm),
        DataType::Date => evaluate_typed(
            starts.date()?.physical(),
            ends.date()?.physical(),
            algorithm,
        ),
        DataType::Datetime(_, _) => evaluate_typed(
            starts.datetime()?.physical(),
            ends.datetime()?.physical(),
            algorithm,
        ),
        dtype => polars_bail!(
            InvalidOperation: "{} requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}",
            name, dtype
        ),
    }
}

fn evaluate_typed<T>(
    starts: &ChunkedArray<T>,
    ends: &ChunkedArray<T>,
    algorithm: Algorithm<'_>,
) -> PolarsResult<Series>
where
    T: PolarsIntegerType,
    T::Native: Ord,
{
    polars_ensure!(
        starts.null_count() == 0 && ends.null_count() == 0,
        ComputeError: "{} does not support null endpoints", algorithm.name()
    );
    // Borrow contiguous inputs; only materialize columns spanning multiple chunks.
    let [starts, ends] = [starts, ends].map(|column| {
        column
            .cont_slice()
            .map(Cow::Borrowed)
            .unwrap_or_else(|_| Cow::Owned(column.into_no_null_iter().collect()))
    });
    match algorithm {
        Algorithm::OverlapCount => {
            let counts = intervals_core::overlap_counts(&starts, &ends)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            let counts: Vec<u64> = counts.into_iter().map(|count| count as u64).collect();
            Ok(Series::from_vec(algorithm.name().into(), counts))
        }
        Algorithm::AssignLanes => {
            let lanes = intervals_core::assign_lanes(&starts, &ends)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::from_vec(algorithm.name().into(), lanes))
        }
        Algorithm::MaxWeight(weights) => {
            let mask = intervals_core::max_weight_non_overlapping(&starts, &ends, weights)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
    }
}
