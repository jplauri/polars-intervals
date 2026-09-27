use super::{integer_values, validate_integer_dtype};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::PySeries;
use std::borrow::Cow;

const NAME: &str = "max_weight_with_capacity_profile";

/// Select a globally maximum-weight subset under a piecewise-constant capacity.
///
/// Jobs and profile segments are half-open. Unsorted profile segments are
/// accepted; overlaps are rejected, and gaps/outside-profile time have capacity
/// zero. Valid empty profile rows have no effect. Every selected non-empty job
/// consumes one capacity unit throughout its lifetime. Positive empty jobs are
/// always selected, while nonpositive weights are omitted.
///
/// Returns a non-null Boolean Series named `selected` in original job row order.
/// Both collections include all chunks and may have different row counts.
/// Endpoints must have exactly the same supported integer, Date or Datetime
/// logical dtype, including time unit and timezone. Weights and capacities are
/// non-null signed/unsigned 8/16/32/64-bit integers, widened exactly to `i128`.
/// Capacities must be nonnegative; values larger than the candidate job count
/// may be clamped without changing feasibility. No cast or broadcasting occurs.
///
/// Constant profiles dispatch to [`super::max_weight_with_capacity`]'s core
/// kernel. See [`intervals_core::max_weight_with_capacity_profile`] for the exact
/// flow formulation, preprocessing, fast paths and complexity. Ties are
/// deterministic for identical inputs; a particular optimum is unspecified.
///
/// # Errors
///
/// Rejects unequal lengths within either collection, nulls, unsupported or
/// mismatched dtypes, reversed jobs/profile rows, overlapping non-empty profile
/// segments, negative capacities, and checked `i128` objective overflow.
pub fn max_weight_with_capacity_profile(
    starts: &Series,
    ends: &Series,
    weights: &Series,
    profile_starts: &Series,
    profile_ends: &Series,
    capacities: &Series,
) -> PolarsResult<Series> {
    validate_integer_dtype(weights.dtype(), NAME, "weight")?;
    validate_integer_dtype(capacities.dtype(), NAME, "capacity")?;
    for (start, end, values, role) in [
        (starts, ends, weights, "jobs"),
        (profile_starts, profile_ends, capacities, "profile"),
    ] {
        polars_ensure!(start.len() == end.len() && start.len() == values.len(),
            ShapeMismatch: "{} requires equal lengths within {}, got {}, {}, {}",
            NAME, role, start.len(), end.len(), values.len());
    }
    let endpoints = [starts, ends, profile_starts, profile_ends];
    polars_ensure!(endpoints.iter().all(|s| s.dtype() == starts.dtype()),
        InvalidOperation: "{} requires matching integer, Date, or Datetime dtypes across jobs and profile (including Datetime time unit and timezone)", NAME);
    polars_ensure!(endpoints.iter().all(|s| s.null_count() == 0),
        ComputeError: "{} does not support null endpoints", NAME);
    polars_ensure!(weights.null_count() == 0, ComputeError:
        "{} does not support null weights", NAME);
    polars_ensure!(capacities.null_count() == 0, ComputeError:
        "{} does not support null capacity", NAME);
    let weights = integer_values(weights)?;
    let capacities = integer_values(capacities)?;
    macro_rules! dispatch {
        ($accessor:ident) => {
            evaluate_typed(
                [
                    starts.$accessor()?,
                    ends.$accessor()?,
                    profile_starts.$accessor()?,
                    profile_ends.$accessor()?,
                ],
                &weights,
                &capacities,
            )
        };
    }
    match starts.dtype() {
        DataType::Int8 => dispatch!(i8),
        DataType::Int16 => dispatch!(i16),
        DataType::Int32 => dispatch!(i32),
        DataType::Int64 => dispatch!(i64),
        DataType::UInt8 => dispatch!(u8),
        DataType::UInt16 => dispatch!(u16),
        DataType::UInt32 => dispatch!(u32),
        DataType::UInt64 => dispatch!(u64),
        DataType::Date => evaluate_typed(
            [
                starts.date()?.physical(),
                ends.date()?.physical(),
                profile_starts.date()?.physical(),
                profile_ends.date()?.physical(),
            ],
            &weights,
            &capacities,
        ),
        DataType::Datetime(_, _) => evaluate_typed(
            [
                starts.datetime()?.physical(),
                ends.datetime()?.physical(),
                profile_starts.datetime()?.physical(),
                profile_ends.datetime()?.physical(),
            ],
            &weights,
            &capacities,
        ),
        dtype => polars_bail!(InvalidOperation:
            "{} requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}", NAME, dtype),
    }
}

fn evaluate_typed<T>(
    endpoints: [&ChunkedArray<T>; 4],
    weights: &[i128],
    capacities: &[i128],
) -> PolarsResult<Series>
where
    T: PolarsIntegerType,
    T::Native: Ord,
{
    let [starts, ends, profile_starts, profile_ends] = endpoints.map(|column| {
        column
            .cont_slice()
            .map(Cow::Borrowed)
            .unwrap_or_else(|_| Cow::Owned(column.into_no_null_iter().collect()))
    });
    let selected = intervals_core::max_weight_with_capacity_profile(
        &starts,
        &ends,
        weights,
        &profile_starts,
        &profile_ends,
        capacities,
    )
    .map_err(|error| polars_err!(ComputeError: "{error}"))?;
    Ok(Series::new("selected".into(), selected))
}

#[pyo3::pyfunction(name = "max_weight_with_capacity_profile")]
pub(crate) fn max_weight_with_capacity_profile_py(
    py: Python<'_>,
    starts: PySeries,
    ends: PySeries,
    weights: PySeries,
    profile_starts: PySeries,
    profile_ends: PySeries,
    capacities: PySeries,
) -> PyResult<PySeries> {
    let result = py.detach(|| {
        max_weight_with_capacity_profile(
            &starts.0,
            &ends.0,
            &weights.0,
            &profile_starts.0,
            &profile_ends.0,
            &capacities.0,
        )
    });
    result.map(PySeries).map_err(|error| {
        // Match the expression API's public Polars exceptions. PyPolarsErr uses
        // its own exception classes, which are not polars.exceptions subclasses.
        let class = match &error {
            PolarsError::InvalidOperation(_) => "InvalidOperationError",
            PolarsError::ShapeMismatch(_) => "ShapeError",
            _ => "ComputeError",
        };
        let exception = py
            .import("polars.exceptions")
            .and_then(|module| module.getattr(class))
            .and_then(|class| class.call1((error.to_string(),)));
        match exception {
            Ok(exception) => PyErr::from_value(exception),
            Err(import_error) => import_error,
        }
    })
}
