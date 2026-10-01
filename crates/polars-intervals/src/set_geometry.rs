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
/// Row errors identify the side and original index, with left-first precedence.
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
    dispatch_endpoints!(left_starts, left_ends, name, |starts, ends| evaluate_typed(
        starts,
        ends,
        left_keys,
        (right_starts, right_ends, right_keys),
        left_starts.dtype(),
        intersection,
    ))
}

fn validated_values<'a, T: PolarsIntegerType>(
    starts: &'a ChunkedArray<T>,
    ends: &'a ChunkedArray<T>,
    side: &str,
) -> PolarsResult<[Cow<'a, [T::Native]>; 2]>
where
    T::Native: Ord,
{
    // Preserve row precedence between nulls and reversed ranges. The common
    // null-free route borrows contiguous arrays and reuses the core validator.
    if starts.null_count() != 0 || ends.null_count() != 0 {
        for (index, (start, end)) in starts.iter().zip(ends.iter()).enumerate() {
            let (Some(start), Some(end)) = (start, end) else {
                polars_bail!(ComputeError: "{}: null endpoints at index {}", side, index);
            };
            if start > end {
                polars_bail!(ComputeError: "{}: {}", side,
                    intervals_core::IntervalError::InvalidInterval { index });
            }
        }
    }
    let values = endpoint_values(starts, ends, side)?;
    intervals_core::validate_intervals(&values[0], &values[1])
        .map_err(|error| polars_err!(ComputeError: "{side}: {error}"))?;
    Ok(values)
}

fn evaluate_typed<T>(
    left_starts: &ChunkedArray<T>,
    left_ends: &ChunkedArray<T>,
    left_keys: &[Series],
    (right_starts, right_ends, right_keys): (&Series, &Series, &[Series]),
    dtype: &DataType,
    intersection: bool,
) -> PolarsResult<DataFrame>
where
    T: PolarsIntegerType,
    T::Native: Ord,
    ChunkedArray<T>: IntoSeries,
{
    let [left_starts, left_ends] = validated_values(left_starts, left_ends, "left")?;
    let right_starts = right_starts.to_physical_repr();
    let right_ends = right_ends.to_physical_repr();
    let [right_starts, right_ends] = validated_values(
        right_starts.unpack::<T>()?,
        right_ends.unpack::<T>()?,
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
        let len = left_starts.len().checked_add(right_starts.len())
            .filter(|&len| IdxSize::try_from(len).is_ok())
            .ok_or_else(|| polars_err!(ComputeError: "combined interval count exceeds the Polars index range"))?;
        let columns = left_keys
            .iter()
            .zip(right_keys)
            .map(|(left, right)| {
                let mut key = left.clone();
                key.append(right)?;
                Ok(key.into_column())
            })
            .collect::<PolarsResult<Vec<_>>>()?;
        let frame = DataFrame::new(len, columns)?;
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

fn evaluate_tagged(
    starts: &Series,
    ends: &Series,
    keys: &[Series],
    is_left: &Series,
    ordinal: &Series,
    intersection: bool,
) -> PolarsResult<DataFrame> {
    polars_ensure!(is_left.len() == starts.len() && ordinal.len() == starts.len(), ShapeMismatch:
        "set intervals requires equal endpoint, source tag and ordinal lengths");
    validate_endpoint_pair(starts, ends, "set intervals")?;
    validate_group_keys(keys, starts.len(), "set intervals", &["start", "end"])?;
    let is_left = is_left.bool()?;
    let ordinal = ordinal.u64()?;
    polars_ensure!(is_left.null_count() == 0 && ordinal.null_count() == 0, ComputeError:
        "set intervals source tags and ordinals must be non-null");
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (index, (is_left, ordinal)) in is_left
        .no_null_iter()
        .zip(ordinal.into_no_null_iter())
        .enumerate()
    {
        let index = IdxSize::try_from(index).map_err(
            |_| polars_err!(ComputeError: "combined interval count exceeds the Polars index range"),
        )?;
        if is_left {
            left.push((ordinal, index));
        } else {
            right.push((ordinal, index));
        }
    }
    let take = |mut rows: Vec<(u64, IdxSize)>| -> PolarsResult<_> {
        if !rows.is_sorted_by_key(|&(ordinal, _)| ordinal) {
            rows.sort_unstable_by_key(|&(ordinal, _)| ordinal);
        }
        polars_ensure!(rows.iter().enumerate().all(|(index, &(ordinal, _))| ordinal == index as u64), ComputeError:
            "set intervals requires contiguous original side ordinals");
        let indices = IdxCa::from_vec(
            "".into(),
            rows.into_iter().map(|(_, index)| index).collect(),
        );
        Ok((
            starts.take(&indices)?,
            ends.take(&indices)?,
            keys.iter()
                .map(|key| key.take(&indices))
                .collect::<PolarsResult<Vec<_>>>()?,
        ))
    };
    let (ls, le, lk) = take(left)?;
    let (rs, re, rk) = take(right)?;
    evaluate((&ls, &le, &lk), (&rs, &re, &rk), intersection)
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

#[pyo3::pyfunction(name = "set_intervals_tagged")]
pub(crate) fn set_intervals_tagged_py(
    py: Python<'_>,
    starts: PySeries,
    ends: PySeries,
    keys: Vec<PySeries>,
    is_left: PySeries,
    ordinal: PySeries,
    intersection: bool,
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        evaluate_tagged(
            &starts.0,
            &ends.0,
            &keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            &is_left.0,
            &ordinal.0,
            intersection,
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_rows_restore_each_sources_order() {
        let starts = Series::new("start".into(), [4i64, 20, 0, 2, 10]);
        let ends = Series::new("end".into(), [6i64, 22, 8, 4, 12]);
        let keys = [Series::new("key".into(), ["a", "b", "a", "a", "b"])];
        let side = Series::new("side".into(), [false, true, true, false, true]);
        let ordinals = Series::new("ordinal".into(), [1u64, 0, 1, 0, 2]);
        let result = evaluate_tagged(&starts, &ends, &keys, &side, &ordinals, false).unwrap();
        assert_eq!(
            result["key"].str().unwrap().iter().collect::<Vec<_>>(),
            [Some("b"), Some("b"), Some("a"), Some("a")]
        );
        assert_eq!(
            result["start"]
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [10, 20, 0, 6]
        );
        assert_eq!(
            result["end"]
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [12, 22, 2, 8]
        );
        let reversed = Series::new("end".into(), [3i64, 22, 8, 4, 12]);
        let error =
            evaluate_tagged(&starts, &reversed, &keys, &side, &ordinals, false).unwrap_err();
        assert!(
            error.to_string().contains("right: interval at index 1"),
            "{error}"
        );
    }
}
