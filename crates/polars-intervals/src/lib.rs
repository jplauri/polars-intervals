//! Interval algorithms for Rust Polars, backed by `intervals-core`.
//!
//! [`overlap_count`] and [`assign_lanes`] adapt integer, Date, and Datetime
//! [`Series`] to the core. Python exposes both as Polars expression plugins.

use polars::prelude::*;
use std::borrow::Cow;

#[pyo3::pymodule]
mod _internal {}

// The output_type_func form also catches failures while importing field dtypes.
// The constant output_type form can abort at the FFI boundary for a dtype whose
// optional Polars feature is disabled (e.g. Int128), before our validation runs.
fn count_output(inputs: &[Field]) -> PolarsResult<Field> {
    output_field(inputs, "overlap_count", DataType::UInt64)
}

fn lanes_output(inputs: &[Field]) -> PolarsResult<Field> {
    output_field(inputs, "assign_lanes", DataType::UInt32)
}

fn output_field(inputs: &[Field], name: &str, dtype: DataType) -> PolarsResult<Field> {
    polars_ensure!(
        inputs.len() == 2,
        InvalidOperation: "{} requires exactly two inputs, got {}", name, inputs.len()
    );
    Ok(Field::new(inputs[0].name().clone(), dtype))
}

#[pyo3_polars::derive::polars_expr(output_type_func = count_output)]
fn overlap_count_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    polars_ensure!(
        inputs.len() == 2,
        InvalidOperation: "overlap_count requires exactly two inputs, got {}",
        inputs.len()
    );
    overlap_count(&inputs[0], &inputs[1])
}

#[pyo3_polars::derive::polars_expr(output_type_func = lanes_output)]
fn assign_lanes_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    polars_ensure!(
        inputs.len() == 2,
        InvalidOperation: "assign_lanes requires exactly two inputs, got {}",
        inputs.len()
    );
    assign_lanes(&inputs[0], &inputs[1])
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

#[derive(Clone, Copy)]
enum Algorithm {
    OverlapCount,
    AssignLanes,
}

impl Algorithm {
    fn name(self) -> &'static str {
        match self {
            Self::OverlapCount => "overlap_count",
            Self::AssignLanes => "assign_lanes",
        }
    }
}

fn evaluate(starts: &Series, ends: &Series, algorithm: Algorithm) -> PolarsResult<Series> {
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
    algorithm: Algorithm,
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
    let starts = starts
        .cont_slice()
        .map(Cow::Borrowed)
        .unwrap_or_else(|_| Cow::Owned(starts.into_no_null_iter().collect()));
    let ends = ends
        .cont_slice()
        .map(Cow::Borrowed)
        .unwrap_or_else(|_| Cow::Owned(ends.into_no_null_iter().collect()));
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
    }
}
