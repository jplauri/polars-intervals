use super::set_geometry::{combined_key_frame, validated_values};
use super::{validate_endpoint_pair, validate_group_keys};
use intervals_core::CoverageStats;
use polars::prelude::*;
use pyo3::prelude::*;
use pyo3_polars::{PyDataFrame, PySeries};

const NAME: &str = "coverage_stats";
const OUTPUT_NAMES: [&str; 4] = [
    "overlap_count",
    "covered_length",
    "query_length",
    "covered_fraction",
];

/// Measure source-record count and union coverage separately for every query.
///
/// Returns only the four statistics, in global original query order. The Python
/// wrapper attaches them to the original query frame without importing payloads
/// through the native geometry binding. All four endpoints must have identical
/// integer, Date, or Datetime dtypes. Groups match exact key tuples, including
/// nulls. All chunks form one collection, and unmatched queries remain present.
///
/// `overlap_count` is non-null `UInt64`. The non-null `Int128` `covered_length`
/// and `query_length` use physical integer units, days, or Datetime ticks.
/// `covered_fraction` is `Float64`, null only for empty queries. Duplicate
/// sources count independently but their covered coordinates count once.
/// A query used as its own source includes itself when nonempty.
/// See [`intervals_core::coverage_stats`] for exact arithmetic and complexity.
///
/// # Errors
/// Rejects unequal endpoint/key lengths within either operand, unsupported or
/// mismatched logical dtypes, invalid keys, nulls, and reversed intervals.
/// Every original row validates before grouping or shortcuts, including
/// source-only groups. Row errors name `queries` or `intervals` and the original
/// index in that operand. Keys may be named `start` or `end`, but cannot collide
/// with the four statistic names.
pub fn coverage_stats(
    query_starts: &Series,
    query_ends: &Series,
    query_keys: &[Series],
    interval_starts: &Series,
    interval_ends: &Series,
    interval_keys: &[Series],
) -> PolarsResult<DataFrame> {
    validate_schema(
        (query_starts, query_ends, query_keys),
        (interval_starts, interval_ends, interval_keys),
    )?;
    dispatch_endpoints!(query_starts.dtype(), NAME, |cast| evaluate_typed(
        (cast(query_starts)?, cast(query_ends)?, query_keys),
        (cast(interval_starts)?, cast(interval_ends)?, interval_keys),
    ))
}

fn validate_schema(
    (query_starts, query_ends, query_keys): (&Series, &Series, &[Series]),
    (interval_starts, interval_ends, interval_keys): (&Series, &Series, &[Series]),
) -> PolarsResult<()> {
    validate_endpoint_pair(query_starts, query_ends, "coverage_stats queries")?;
    validate_endpoint_pair(interval_starts, interval_ends, "coverage_stats intervals")?;
    polars_ensure!(query_starts.dtype() == interval_starts.dtype(), InvalidOperation:
        "{} requires matching integer, Date, or Datetime dtypes across queries and intervals (including Datetime time unit and timezone), got {} and {}",
        NAME, query_starts.dtype(), interval_starts.dtype());
    validate_group_keys(
        query_keys,
        query_starts.len(),
        "coverage_stats queries",
        &OUTPUT_NAMES,
    )?;
    validate_group_keys(
        interval_keys,
        interval_starts.len(),
        "coverage_stats intervals",
        &OUTPUT_NAMES,
    )?;
    polars_ensure!(query_keys.len() == interval_keys.len(), InvalidOperation:
        "{} requires matching queries and intervals group keys", NAME);
    for (query, interval) in query_keys.iter().zip(interval_keys) {
        polars_ensure!(query.name() == interval.name() && query.dtype() == interval.dtype(), InvalidOperation:
            "{} requires matching queries and intervals group key names and logical dtypes", NAME);
    }
    Ok(())
}

fn evaluate_typed<T>(
    (query_starts, query_ends, query_keys): (&ChunkedArray<T>, &ChunkedArray<T>, &[Series]),
    (interval_starts, interval_ends, interval_keys): (
        &ChunkedArray<T>,
        &ChunkedArray<T>,
        &[Series],
    ),
) -> PolarsResult<DataFrame>
where
    T: PolarsIntegerType,
    T::Native: intervals_core::CoverageEndpoint,
{
    let [query_starts, query_ends] = validated_values(query_starts, query_ends, NAME, "queries")?;
    let [interval_starts, interval_ends] =
        validated_values(interval_starts, interval_ends, NAME, "intervals")?;
    let solve = |qs: &[T::Native], qe: &[T::Native], ss: &[T::Native], se: &[T::Native]| {
        intervals_core::coverage_stats(qs, qe, ss, se)
            .map_err(|error| polars_err!(ComputeError: "{error}"))
    };
    let output = if query_starts.is_empty() {
        Vec::new()
    } else if query_keys.is_empty() {
        solve(&query_starts, &query_ends, &interval_starts, &interval_ends)?
    } else {
        let frame = combined_key_frame(
            query_keys,
            interval_keys,
            query_starts.len(),
            interval_starts.len(),
        )?;
        let groups = frame.group_by_stable(query_keys.iter().map(|key| key.name().as_str()))?;
        let empty = CoverageStats {
            overlap_count: 0,
            covered_length: 0,
            query_length: 0,
            covered_fraction: None,
        };
        let mut output = vec![empty; query_starts.len()];
        let (mut qs, mut qe, mut ss, mut se, mut query_rows) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for group in groups.get_groups().iter() {
            // Both original operands validated before source-only groups disappear.
            if group.first() as usize >= query_starts.len() {
                continue;
            }
            qs.clear();
            qe.clear();
            ss.clear();
            se.clear();
            query_rows.clear();
            let mut gather = |row: IdxSize| {
                let row = row as usize;
                if row < query_starts.len() {
                    qs.push(query_starts[row]);
                    qe.push(query_ends[row]);
                    query_rows.push(row);
                } else {
                    let row = row - query_starts.len();
                    ss.push(interval_starts[row]);
                    se.push(interval_ends[row]);
                }
            };
            match &group {
                GroupsIndicator::Idx((_, rows)) => rows.iter().copied().for_each(gather),
                GroupsIndicator::Slice([first, len]) => {
                    (*first..*first + *len).for_each(&mut gather)
                }
            }
            for (&row, stats) in query_rows.iter().zip(solve(&qs, &qe, &ss, &se)?) {
                output[row] = stats;
            }
        }
        output
    };
    DataFrame::new(
        output.len(),
        vec![
            UInt64Chunked::from_iter_values(
                OUTPUT_NAMES[0].into(),
                output.iter().map(|s| s.overlap_count),
            )
            .into_column(),
            Int128Chunked::from_iter_values(
                OUTPUT_NAMES[1].into(),
                output.iter().map(|s| s.covered_length),
            )
            .into_column(),
            Int128Chunked::from_iter_values(
                OUTPUT_NAMES[2].into(),
                output.iter().map(|s| s.query_length),
            )
            .into_column(),
            Float64Chunked::from_iter_options(
                OUTPUT_NAMES[3].into(),
                output.into_iter().map(|s| s.covered_fraction),
            )
            .into_column(),
        ],
    )
}

#[pyo3::pyfunction(name = "coverage_stats")]
pub(crate) fn coverage_stats_py(
    py: Python<'_>,
    query_starts: PySeries,
    query_ends: PySeries,
    query_keys: Vec<PySeries>,
    interval_starts: PySeries,
    interval_ends: PySeries,
    interval_keys: Vec<PySeries>,
) -> PyResult<PyDataFrame> {
    py.detach(|| {
        coverage_stats(
            &query_starts.0,
            &query_ends.0,
            &query_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            &interval_starts.0,
            &interval_ends.0,
            &interval_keys
                .into_iter()
                .map(|key| key.0)
                .collect::<Vec<_>>(),
        )
    })
    .map(PyDataFrame)
    .map_err(|error| super::profile::python_error(py, error))
}

// The private native-Polars benchmark includes the same validation cost and
// original-row diagnostics without preparing a source index it will not use.
#[pyo3::pyfunction(name = "validate_coverage_stats")]
pub(crate) fn validate_coverage_stats_py(
    py: Python<'_>,
    query_starts: PySeries,
    query_ends: PySeries,
    query_keys: Vec<PySeries>,
    interval_starts: PySeries,
    interval_ends: PySeries,
    interval_keys: Vec<PySeries>,
) -> PyResult<()> {
    py.detach(|| {
        validate_schema(
            (
                &query_starts.0,
                &query_ends.0,
                &query_keys.into_iter().map(|key| key.0).collect::<Vec<_>>(),
            ),
            (
                &interval_starts.0,
                &interval_ends.0,
                &interval_keys
                    .into_iter()
                    .map(|key| key.0)
                    .collect::<Vec<_>>(),
            ),
        )?;
        dispatch_endpoints!(query_starts.0.dtype(), NAME, |cast| {
            validated_values(
                cast(&query_starts.0)?,
                cast(&query_ends.0)?,
                NAME,
                "queries",
            )?;
            validated_values(
                cast(&interval_starts.0)?,
                cast(&interval_ends.0)?,
                NAME,
                "intervals",
            )?;
            Ok(())
        })
    })
    .map_err(|error| super::profile::python_error(py, error))
}
