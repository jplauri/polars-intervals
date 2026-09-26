//! Interval algorithms for Rust Polars, backed by `intervals-core`.
//!
//! [`overlap_count`] adapts integer, Date, and Datetime [`Series`] to the core.
//! The Python package exposes the same operation as a Polars expression plugin.

use polars::prelude::*;
use std::borrow::Cow;

#[pyo3::pymodule]
mod _internal {}

#[pyo3_polars::derive::polars_expr(output_type = UInt64)]
fn overlap_count_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    polars_ensure!(
        inputs.len() == 2,
        InvalidOperation: "overlap_count requires exactly two inputs, got {}",
        inputs.len()
    );
    overlap_count(&inputs[0], &inputs[1])
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
    polars_ensure!(
        starts.len() == ends.len(),
        ShapeMismatch: "overlap_count requires equal lengths, got {} starts and {} ends",
        starts.len(), ends.len()
    );
    polars_ensure!(
        starts.dtype() == ends.dtype(),
        InvalidOperation: "overlap_count requires matching integer, Date, or Datetime dtypes (including Datetime time unit and timezone), got {} and {}",
        starts.dtype(), ends.dtype()
    );
    match starts.dtype() {
        DataType::Int8 => count_typed(starts.i8()?, ends.i8()?),
        DataType::Int16 => count_typed(starts.i16()?, ends.i16()?),
        DataType::Int32 => count_typed(starts.i32()?, ends.i32()?),
        DataType::Int64 => count_typed(starts.i64()?, ends.i64()?),
        DataType::UInt8 => count_typed(starts.u8()?, ends.u8()?),
        DataType::UInt16 => count_typed(starts.u16()?, ends.u16()?),
        DataType::UInt32 => count_typed(starts.u32()?, ends.u32()?),
        DataType::UInt64 => count_typed(starts.u64()?, ends.u64()?),
        DataType::Date => count_typed(starts.date()?.physical(), ends.date()?.physical()),
        DataType::Datetime(_, _) => {
            count_typed(starts.datetime()?.physical(), ends.datetime()?.physical())
        }
        dtype => polars_bail!(
            InvalidOperation: "overlap_count requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}",
            dtype
        ),
    }
}

fn count_typed<T>(starts: &ChunkedArray<T>, ends: &ChunkedArray<T>) -> PolarsResult<Series>
where
    T: PolarsIntegerType,
    T::Native: Ord,
{
    polars_ensure!(
        starts.null_count() == 0 && ends.null_count() == 0,
        ComputeError: "overlap_count does not support null endpoints"
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
    let counts = intervals_core::overlap_counts(&starts, &ends)
        .map_err(|error| polars_err!(ComputeError: "{error}"))?;
    let counts: Vec<u64> = counts.into_iter().map(|count| count as u64).collect();
    Ok(Series::from_vec("overlap_count".into(), counts))
}
