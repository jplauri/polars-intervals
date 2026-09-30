use super::cover::{TargetValue, physical_domain, scalar_physical};
use super::{
    endpoint_values, integer_values, logical_series, native_domain, validate_endpoint_pair,
    validate_group_keys, validate_integer_dtype,
};
use intervals_core::{CoverageSegment, IntervalError};
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::{PyDataFrame, PySeries};

const NAME: &str = "coverage_profile";

/// Compute the canonical half-open coverage/load function as new segment rows.
///
/// Omitted weights mean units, without allocating a ones column. Explicit
/// weights are nonnegative 8/16/32/64-bit integers. Output columns are the
/// supplied keys, `start`, `end`, and non-null `Int128` `load`. Endpoint/key
/// logical dtypes and Datetime metadata are preserved exactly.
///
/// All chunks form one collection per key tuple. Native grouping retains null
/// keys and first-appearance order. With no keys there is one collection even
/// for empty input; grouped empty input has no groups. Key names must be distinct
/// and cannot be `start`, `end`, or `load`.
///
/// An optional scalar domain clips contributions after every row is validated.
/// Otherwise each group's nonempty intervals, including zero weights, establish
/// its domain. Empty intervals never establish a domain or contribute. Full
/// mode partitions that domain; sparse mode omits zero segments. Equal touching
/// loads are coalesced. See [`intervals_core::coverage_profile`] for the sweep,
/// checked arithmetic and `O(n log n + z)` time / `O(n + z)` space bounds.
///
/// # Errors
/// Rejects unequal lengths, null endpoints/weights, unsupported or mismatched
/// dtypes, invalid grouping keys, reversed intervals/domains, negative weights,
/// and unrepresentable loads inside the domain. Row errors refer to original
/// input indices, including rows that are empty, zero, or clipped away.
pub fn coverage_profile(
    starts: &Series,
    ends: &Series,
    weights: Option<&Series>,
    keys: &[Series],
    domain: Option<(&Scalar, &Scalar)>,
    include_zero: bool,
) -> PolarsResult<DataFrame> {
    let domain = domain
        .map(|(left, right)| -> PolarsResult<_> {
            Ok((
                scalar_physical(left, starts.dtype())?,
                scalar_physical(right, starts.dtype())?,
            ))
        })
        .transpose()?;
    evaluate(starts, ends, weights, keys, domain, include_zero)
}

fn evaluate(
    starts: &Series,
    ends: &Series,
    weights: Option<&Series>,
    keys: &[Series],
    domain: Option<(i128, i128)>,
    include_zero: bool,
) -> PolarsResult<DataFrame> {
    validate_endpoint_pair(starts, ends, NAME)?;
    let weights = weights
        .map(|weights| {
            validate_integer_dtype(weights.dtype(), NAME, "weight")?;
            polars_ensure!(weights.len() == starts.len(), ShapeMismatch:
                "{} requires equal interval and weight lengths", NAME);
            polars_ensure!(weights.null_count() == 0, ComputeError:
                "{} does not support null weights", NAME);
            integer_values(weights)
        })
        .transpose()?;
    validate_group_keys(keys, starts.len(), NAME, &["start", "end", "load"])?;
    dispatch_endpoints!(starts, ends, NAME, |left, right| evaluate_typed(
        left,
        right,
        weights.as_deref(),
        keys,
        domain,
        include_zero,
        starts.dtype(),
    ))
}

fn evaluate_typed<T>(
    starts: &ChunkedArray<T>,
    ends: &ChunkedArray<T>,
    weights: Option<&[i128]>,
    keys: &[Series],
    domain: Option<(i128, i128)>,
    include_zero: bool,
    dtype: &DataType,
) -> PolarsResult<DataFrame>
where
    T: PolarsIntegerType,
    T::Native: Ord + TryFrom<i128>,
    ChunkedArray<T>: IntoSeries,
{
    let [starts, ends] = endpoint_values(starts, ends, NAME)?;
    let domain = native_domain(domain)?;
    // Validate before partitioning, so row diagnostics retain original indices
    // even for groups with no output. Ungrouped calls use core validation directly.
    if !keys.is_empty() {
        for (index, (&start, &end)) in starts.iter().zip(ends.iter()).enumerate() {
            if start > end {
                return Err(
                    polars_err!(ComputeError: "{}", IntervalError::InvalidInterval { index }),
                );
            }
            if weights.is_some_and(|weights| weights[index] < 0) {
                return Err(polars_err!(ComputeError: "{}", IntervalError::NegativeLoad { index }));
            }
        }
    }
    if domain.is_some_and(|(left, right)| left > right) {
        return Err(polars_err!(ComputeError: "{}", IntervalError::InvalidDomain));
    }
    let solve = |starts: &[T::Native], ends: &[T::Native], weights: Option<&[i128]>| {
        match weights {
            Some(weights) => intervals_core::weighted_coverage_profile(
                starts,
                ends,
                weights,
                domain,
                include_zero,
            ),
            None => intervals_core::coverage_profile(starts, ends, domain, include_zero),
        }
        .map_err(|error| polars_err!(ComputeError: "{error}"))
    };
    let mut output = Vec::<CoverageSegment<T::Native>>::new();
    let mut key_rows = Vec::new();
    if keys.is_empty() {
        output = solve(&starts, &ends, weights)?;
    } else {
        let frame = DataFrame::new(
            starts.len(),
            keys.iter().cloned().map(IntoColumn::into_column).collect(),
        )?;
        let groups = frame.group_by_stable(keys.iter().map(|key| key.name().as_str()))?;
        // Reuse scratch across groups; each gather includes all chunks.
        let (mut group_starts, mut group_ends, mut group_weights) =
            (Vec::new(), Vec::new(), Vec::new());
        for group in groups.get_groups().iter() {
            group_starts.clear();
            group_ends.clear();
            group_weights.clear();
            let mut gather = |row: IdxSize| {
                let row = row as usize;
                group_starts.push(starts[row]);
                group_ends.push(ends[row]);
                if let Some(weights) = weights {
                    group_weights.push(weights[row]);
                }
            };
            match &group {
                GroupsIndicator::Idx((_, rows)) => rows.iter().copied().for_each(gather),
                GroupsIndicator::Slice([first, len]) => {
                    (*first..*first + *len).for_each(&mut gather);
                }
            }
            let segments = solve(
                &group_starts,
                &group_ends,
                weights.map(|_| group_weights.as_slice()),
            )?;
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
        (
            "start",
            output.iter().map(|segment| segment.start).collect(),
        ),
        ("end", output.iter().map(|segment| segment.end).collect()),
    ] {
        let physical = ChunkedArray::<T>::from_vec(name.into(), values).into_series();
        columns.push(logical_series(physical, dtype)?.into_column());
    }
    let height = output.len();
    columns.push(
        Int128Chunked::from_iter_values(
            "load".into(),
            output.into_iter().map(|segment| segment.load),
        )
        .into_column(),
    );
    DataFrame::new(height, columns)
}

#[pyo3::pyfunction(name = "coverage_profile", signature = (starts, ends, weights, keys, domain, include_zero))]
pub(crate) fn coverage_profile_py(
    py: Python<'_>,
    starts: PySeries,
    ends: PySeries,
    weights: Option<PySeries>,
    keys: Vec<PySeries>,
    domain: Option<(TargetValue, TargetValue)>,
    include_zero: bool,
) -> PyResult<PyDataFrame> {
    let result = py.detach(|| {
        let domain = physical_domain(domain, starts.0.dtype())?;
        evaluate(
            &starts.0,
            &ends.0,
            weights.as_ref().map(|weights| &weights.0),
            &keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            domain,
            include_zero,
        )
    });
    result
        .map(PyDataFrame)
        .map_err(|error| super::profile::python_error(py, error))
}
