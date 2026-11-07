use super::cover::{TargetValue, physical_domain};
use super::{
    endpoint_values, logical_series, native_domain, validate_endpoint_pair, validate_group_keys,
};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::{PyDataFrame, PySeries};

/// Return the canonical exact union of nonempty half-open intervals.
pub(crate) fn merge_intervals(
    starts: &Series,
    ends: &Series,
    keys: &[Series],
) -> PolarsResult<DataFrame> {
    evaluate(starts, ends, keys, None)
}

fn evaluate(
    starts: &Series,
    ends: &Series,
    keys: &[Series],
    domain: Option<(i128, i128)>,
) -> PolarsResult<DataFrame> {
    let name = if domain.is_some() {
        "interval_gaps"
    } else {
        "merge_intervals"
    };
    validate_endpoint_pair(starts, ends, name)?;
    validate_group_keys(keys, starts.len(), name, &["start", "end"])?;
    dispatch_endpoints!(starts.dtype(), name, |cast| evaluate_typed(
        cast(starts)?,
        cast(ends)?,
        keys,
        domain,
        starts.dtype(),
        name,
    ))
}

fn evaluate_typed<T>(
    starts: &ChunkedArray<T>,
    ends: &ChunkedArray<T>,
    keys: &[Series],
    domain: Option<(i128, i128)>,
    dtype: &DataType,
    name: &str,
) -> PolarsResult<DataFrame>
where
    T: PolarsIntegerType,
    T::Native: Ord + TryFrom<i128>,
    ChunkedArray<T>: IntoSeries,
{
    let [starts, ends] = endpoint_values(starts, ends, name)?;
    // Validate before grouping/clipping so diagnostics always name original rows.
    intervals_core::validate_intervals(&starts, &ends).map_err(super::compute_error)?;
    let domain = native_domain(domain)?;
    if domain.is_some_and(|(left, right)| left > right) {
        return Err(polars_err!(ComputeError: "{}", intervals_core::IntervalError::InvalidDomain));
    }
    let solve = |starts: &[T::Native], ends: &[T::Native]| {
        match domain {
            Some((left, right)) => intervals_core::interval_gaps(starts, ends, left, right),
            None => intervals_core::merge_intervals(starts, ends),
        }
        .map_err(super::compute_error)
    };
    let mut output = Vec::new();
    let mut key_rows = Vec::new();
    if keys.is_empty() {
        output = solve(&starts, &ends)?;
    } else {
        let frame = DataFrame::new(
            starts.len(),
            keys.iter().cloned().map(IntoColumn::into_column).collect(),
        )?;
        let groups = frame.group_by_stable(keys.iter().map(|key| key.name().as_str()))?;
        let (mut group_starts, mut group_ends) = (Vec::new(), Vec::new());
        for group in groups.get_groups().iter() {
            group_starts.clear();
            group_ends.clear();
            let mut gather = |row: IdxSize| {
                group_starts.push(starts[row as usize]);
                group_ends.push(ends[row as usize]);
            };
            match &group {
                GroupsIndicator::Idx((_, rows)) => rows.iter().copied().for_each(gather),
                GroupsIndicator::Slice([first, len]) => {
                    (*first..*first + *len).for_each(&mut gather);
                }
            }
            let segments = solve(&group_starts, &group_ends)?;
            key_rows.extend(std::iter::repeat_n(group.first(), segments.len()));
            output.extend(segments);
        }
    }
    let key_rows = IdxCa::from_vec("".into(), key_rows);
    let mut columns = keys
        .iter()
        .map(|key| key.take(&key_rows).map(IntoColumn::into_column))
        .collect::<PolarsResult<Vec<_>>>()?;
    for (name, values) in [
        ("start", output.iter().map(|&(start, _)| start).collect()),
        ("end", output.iter().map(|&(_, end)| end).collect()),
    ] {
        let physical = ChunkedArray::<T>::from_vec(name.into(), values).into_series();
        columns.push(logical_series(physical, dtype)?.into_column());
    }
    DataFrame::new(output.len(), columns)
}

#[pyo3::pyfunction(name = "merge_intervals")]
pub(crate) fn merge_intervals_py(
    py: Python<'_>,
    starts: PySeries,
    ends: PySeries,
    keys: Vec<PySeries>,
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        merge_intervals(
            &starts.0,
            &ends.0,
            &keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}

#[pyo3::pyfunction(name = "interval_gaps")]
pub(crate) fn interval_gaps_py(
    py: Python<'_>,
    starts: PySeries,
    ends: PySeries,
    keys: Vec<PySeries>,
    domain: (TargetValue, TargetValue),
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        let domain = physical_domain(Some(domain), starts.0.dtype())?;
        evaluate(
            &starts.0,
            &ends.0,
            &keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            domain,
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}
