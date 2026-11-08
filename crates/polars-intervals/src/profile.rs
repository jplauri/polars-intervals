use super::{endpoint_slices, integer_values, validate_integer_dtype};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::PySeries;

const NAME: &str = "max_weight_with_capacity_profile";

/// Select a globally maximum-weight subset under a piecewise-constant capacity.
pub(crate) fn max_weight_with_capacity_profile(
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
    dispatch_endpoints!(starts.dtype(), NAME, |cast| evaluate_typed(
        [
            cast(starts)?,
            cast(ends)?,
            cast(profile_starts)?,
            cast(profile_ends)?
        ],
        &weights,
        &capacities,
    ))
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
    let [starts, ends, profile_starts, profile_ends] = endpoint_slices(endpoints);
    let selected = intervals_core::max_weight_with_capacity_profile(
        &starts,
        &ends,
        weights,
        &profile_starts,
        &profile_ends,
        capacities,
    )
    .map_err(super::compute_error)?;
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
    match result {
        Ok(series) => Ok(PySeries(series)),
        Err(error) => Err(python_error(py, error)),
    }
}

pub(crate) fn python_error(py: Python<'_>, error: PolarsError) -> PyErr {
    // Match the expression API's public Polars exceptions. PyPolarsErr uses
    // its own exception classes, which are not polars.exceptions subclasses.
    let class = match &error {
        PolarsError::InvalidOperation(_) => "InvalidOperationError",
        PolarsError::ShapeMismatch(_) => "ShapeError",
        _ => "ComputeError",
    };
    match py
        .import("polars.exceptions")
        .and_then(|module| module.getattr(class)?.call1((error.to_string(),)))
    {
        Ok(exception) => PyErr::from_value(exception),
        Err(error) => error,
    }
}
