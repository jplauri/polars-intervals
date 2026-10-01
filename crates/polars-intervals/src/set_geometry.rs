use super::{endpoint_values, logical_series, validate_endpoint_pair, validate_group_keys};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::{PyDataFrame, PySeries};
use std::borrow::Cow;

/// Return the canonical half-open set difference of two interval collections.
///
/// Each collection represents its union. Empty intervals contribute nothing.
/// Output columns are the supplied keys, `start`, and `end`. All four endpoint
/// dtypes must match exactly. Groups match by key tuple, including nulls, and
/// retain first appearance in the original left input. Right-only groups emit
/// no rows. All chunks are processed together. See [`intervals_core::subtract_intervals`].
///
/// # Errors
/// Rejects unequal endpoint/key lengths within either side, null endpoints,
/// unsupported or mismatched logical dtypes, reversed intervals, and invalid
/// keys. Every row validates before grouping or skipping either operand.
/// Errors name the side, and the left side validates first. Reversed-row
/// errors also give the original index within that side.
pub fn subtract_intervals(
    left_starts: &Series,
    left_ends: &Series,
    left_keys: &[Series],
    right_starts: &Series,
    right_ends: &Series,
    right_keys: &[Series],
) -> PolarsResult<DataFrame> {
    evaluate(
        (left_starts, left_ends, left_keys),
        (right_starts, right_ends, right_keys),
        false,
    )
}

/// Return the canonical geometric intersection of two interval unions.
///
/// Touching alone has empty intersection. Empty intervals contribute nothing.
/// Schema, validation and grouping follow [`subtract_intervals`]. Swapping the
/// inputs preserves keyed geometry but may change left-oriented group order.
/// See [`intervals_core::intersect_intervals`].
///
/// # Errors
/// Follows [`subtract_intervals`], including validation of unmatched groups.
pub fn intersect_intervals(
    left_starts: &Series,
    left_ends: &Series,
    left_keys: &[Series],
    right_starts: &Series,
    right_ends: &Series,
    right_keys: &[Series],
) -> PolarsResult<DataFrame> {
    evaluate(
        (left_starts, left_ends, left_keys),
        (right_starts, right_ends, right_keys),
        true,
    )
}

fn evaluate(
    (left_starts, left_ends, left_keys): (&Series, &Series, &[Series]),
    (right_starts, right_ends, right_keys): (&Series, &Series, &[Series]),
    intersection: bool,
) -> PolarsResult<DataFrame> {
    let name = if intersection {
        "intersect_intervals"
    } else {
        "subtract_intervals"
    };
    validate_endpoint_pair(left_starts, left_ends, &format!("{name} left"))?;
    validate_endpoint_pair(right_starts, right_ends, &format!("{name} right"))?;
    polars_ensure!(left_starts.dtype() == right_starts.dtype(), InvalidOperation:
        "{} requires matching integer, Date, or Datetime dtypes across left and right (including Datetime time unit and timezone), got {} and {}",
        name, left_starts.dtype(), right_starts.dtype());
    validate_group_keys(left_keys, left_starts.len(), name, &["start", "end"])?;
    validate_group_keys(right_keys, right_starts.len(), name, &["start", "end"])?;
    polars_ensure!(left_keys.len() == right_keys.len(), InvalidOperation:
        "{} requires matching left and right group keys", name);
    for (left, right) in left_keys.iter().zip(right_keys) {
        polars_ensure!(left.name() == right.name() && left.dtype() == right.dtype(), InvalidOperation:
            "{} requires matching left and right group key names and logical dtypes", name);
    }
    dispatch_endpoints!(left_starts.dtype(), name, |cast| evaluate_typed(
        cast(left_starts)?,
        cast(left_ends)?,
        left_keys,
        (right_starts, right_ends, right_keys),
        left_starts.dtype(),
        name,
        intersection,
    ))
}

pub(super) fn validated_values<'a, T: PolarsIntegerType>(
    starts: &'a ChunkedArray<T>,
    ends: &'a ChunkedArray<T>,
    name: &str,
    side: &str,
) -> PolarsResult<[Cow<'a, [T::Native]>; 2]>
where
    T::Native: Ord,
{
    let values = endpoint_values(starts, ends, &format!("{name} {side}"))?;
    intervals_core::validate_intervals(&values[0], &values[1])
        .map_err(|error| polars_err!(ComputeError: "{side}: {error}"))?;
    Ok(values)
}

pub(super) fn combined_key_frame(
    left_keys: &[Series],
    right_keys: &[Series],
    left_len: usize,
    right_len: usize,
) -> PolarsResult<DataFrame> {
    let len = left_len
        .checked_add(right_len)
        .filter(|&len| IdxSize::try_from(len).is_ok())
        .ok_or_else(
            || polars_err!(ComputeError: "combined interval count exceeds the Polars index range"),
        )?;
    let columns = left_keys
        .iter()
        .zip(right_keys)
        .map(|(left, right)| {
            let mut key = left.clone();
            key.append(right)?;
            Ok(key.into_column())
        })
        .collect::<PolarsResult<Vec<_>>>()?;
    DataFrame::new(len, columns)
}

fn evaluate_typed<T>(
    left_starts: &ChunkedArray<T>,
    left_ends: &ChunkedArray<T>,
    left_keys: &[Series],
    (right_starts, right_ends, right_keys): (&Series, &Series, &[Series]),
    dtype: &DataType,
    name: &str,
    intersection: bool,
) -> PolarsResult<DataFrame>
where
    T: PolarsIntegerType,
    T::Native: Ord,
    ChunkedArray<T>: IntoSeries,
{
    let [left_starts, left_ends] = validated_values(left_starts, left_ends, name, "left")?;
    let right_starts = right_starts.to_physical_repr();
    let right_ends = right_ends.to_physical_repr();
    let [right_starts, right_ends] = validated_values(
        right_starts.unpack::<T>()?,
        right_ends.unpack::<T>()?,
        name,
        "right",
    )?;
    let solve = |ls: &[T::Native], le: &[T::Native], rs: &[T::Native], re: &[T::Native]| {
        if intersection {
            intervals_core::intersect_intervals(ls, le, rs, re)
        } else {
            intervals_core::subtract_intervals(ls, le, rs, re)
        }
        .map_err(|error| polars_err!(ComputeError: "{error}"))
    };
    let mut output = Vec::new();
    let mut key_rows = Vec::new();
    if left_keys.is_empty() {
        output = solve(&left_starts, &left_ends, &right_starts, &right_ends)?;
    } else {
        let frame =
            combined_key_frame(left_keys, right_keys, left_starts.len(), right_starts.len())?;
        let groups = frame.group_by_stable(left_keys.iter().map(|key| key.name().as_str()))?;
        let (mut ls, mut le, mut rs, mut re) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for group in groups.get_groups().iter() {
            // All original rows were validated before skipping right-only groups.
            if group.first() as usize >= left_starts.len() {
                continue;
            }
            ls.clear();
            le.clear();
            rs.clear();
            re.clear();
            let mut gather = |row: IdxSize| {
                let row = row as usize;
                if row < left_starts.len() {
                    ls.push(left_starts[row]);
                    le.push(left_ends[row]);
                } else {
                    let row = row - left_starts.len();
                    rs.push(right_starts[row]);
                    re.push(right_ends[row]);
                }
            };
            match &group {
                GroupsIndicator::Idx((_, rows)) => rows.iter().copied().for_each(gather),
                GroupsIndicator::Slice([first, len]) => {
                    (*first..*first + *len).for_each(&mut gather)
                }
            }
            let segments = solve(&ls, &le, &rs, &re)?;
            key_rows.extend(std::iter::repeat_n(group.first(), segments.len()));
            output.extend(segments);
        }
    }
    let key_rows = IdxCa::from_vec("".into(), key_rows);
    let mut columns = left_keys
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

#[pyo3::pyfunction(name = "subtract_intervals")]
pub(crate) fn subtract_intervals_py(
    py: Python<'_>,
    left_starts: PySeries,
    left_ends: PySeries,
    left_keys: Vec<PySeries>,
    right_starts: PySeries,
    right_ends: PySeries,
    right_keys: Vec<PySeries>,
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        subtract_intervals(
            &left_starts.0,
            &left_ends.0,
            &left_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            &right_starts.0,
            &right_ends.0,
            &right_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}

#[pyo3::pyfunction(name = "intersect_intervals")]
pub(crate) fn intersect_intervals_py(
    py: Python<'_>,
    left_starts: PySeries,
    left_ends: PySeries,
    left_keys: Vec<PySeries>,
    right_starts: PySeries,
    right_ends: PySeries,
    right_keys: Vec<PySeries>,
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        intersect_intervals(
            &left_starts.0,
            &left_ends.0,
            &left_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            &right_starts.0,
            &right_ends.0,
            &right_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}
