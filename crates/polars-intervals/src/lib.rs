//! Interval algorithms for Rust Polars, backed by `intervals-core`.
//!
//! [`overlap_count`], [`containment_count`], [`nesting_depth`], [`assign_lanes`],
//! [`assign_balanced_lanes`],
//! [`max_weight_clique`], [`max_weight_non_overlapping`] and
//! [`max_weight_with_capacity`] adapt integer, Date, and Datetime [`Series`]
//! to the core and Python expressions.
//! [`minimum_cover`] and [`minimum_cost_cover`] select exact continuous target covers.
//! [`minimum_cost_dominating_set`] selects rows dominating every interval vertex.
//! [`minimum_stabbing_points`] returns one optimal list of discrete hitting points.

use polars::prelude::*;
use std::borrow::Cow;

// Keep physical extraction identical across row-aligned expressions and frame
// adapters. Callers validate logical dtypes before dispatch.
macro_rules! dispatch_endpoints {
    ($dtype:expr, $name:expr, |$cast:ident| $body:expr) => {{
        let (dtype, name) = ($dtype, $name);
        macro_rules! call {
            ($physical:ty, $extract:expr) => {{
                let $cast: fn(&Series) -> PolarsResult<&$physical> = $extract;
                $body
            }};
        }
        match dtype {
            DataType::Int8 => call!(Int8Chunked, Series::i8),
            DataType::Int16 => call!(Int16Chunked, Series::i16),
            DataType::Int32 => call!(Int32Chunked, Series::i32),
            DataType::Int64 => call!(Int64Chunked, Series::i64),
            DataType::UInt8 => call!(UInt8Chunked, Series::u8),
            DataType::UInt16 => call!(UInt16Chunked, Series::u16),
            DataType::UInt32 => call!(UInt32Chunked, Series::u32),
            DataType::UInt64 => call!(UInt64Chunked, Series::u64),
            DataType::Date => call!(Int32Chunked, |series| Ok(series.date()?.physical())),
            DataType::Datetime(_, _) => {
                call!(Int64Chunked, |series| Ok(series.datetime()?.physical()))
            }
            dtype => polars_bail!(InvalidOperation:
                "{} requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}", name, dtype),
        }
    }};
}

mod cover;
pub use cover::{minimum_cost_cover, minimum_cover};
mod domination;
pub use domination::minimum_cost_dominating_set;
mod profile;
pub use profile::max_weight_with_capacity_profile;
mod coverage_profile;
pub use coverage_profile::coverage_profile;
mod coverage_stats;
pub use coverage_stats::coverage_stats;
mod geometry;
pub use geometry::{interval_gaps, merge_intervals};
mod set_geometry;
pub use set_geometry::{intersect_intervals, subtract_intervals};

#[pyo3::pymodule]
mod _internal {
    #[pymodule_export]
    use super::coverage_profile::coverage_profile_py;
    #[pymodule_export]
    use super::coverage_stats::coverage_stats_py;
    #[pymodule_export]
    use super::coverage_stats::validate_coverage_stats_py;
    #[pymodule_export]
    use super::geometry::interval_gaps_py;
    #[pymodule_export]
    use super::geometry::merge_intervals_py;
    #[pymodule_export]
    use super::profile::max_weight_with_capacity_profile_py;
    #[pymodule_export]
    use super::set_geometry::intersect_intervals_py;
    #[pymodule_export]
    use super::set_geometry::subtract_intervals_py;
}

// The output_type_func form also catches failures while importing field dtypes.
// The constant output_type form can abort at the FFI boundary for a dtype whose
// optional Polars feature is disabled, before our validation runs.
fn count_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "overlap_count")?;
    Ok(Field::new(start.name().clone(), DataType::UInt64))
}

fn containment_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "containment_count")?;
    Ok(Field::new(start.name().clone(), DataType::UInt64))
}

fn nesting_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "nesting_depth")?;
    Ok(Field::new(start.name().clone(), DataType::UInt64))
}

fn lanes_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "assign_lanes")?;
    Ok(Field::new(start.name().clone(), DataType::UInt32))
}

fn clusters_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "cluster_intervals")?;
    Ok(Field::new(start.name().clone(), DataType::UInt32))
}

#[derive(serde::Deserialize)]
struct ClusterOptions {
    include_touching: bool,
}

#[pyo3_polars::derive::polars_expr(output_type_func = clusters_output)]
fn cluster_intervals_plugin(inputs: &[Series], kwargs: ClusterOptions) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "cluster_intervals")?;
    cluster_intervals(starts, ends, kwargs.include_touching)
}

/// Label connected interval components in original row order.
///
/// Nonempty half-open intervals connect by strict overlap, or also by touching
/// when `include_touching` is true. Every empty row is an isolated singleton.
/// IDs are non-null UInt32, contiguous from zero, in first-original-row order.
/// Matching integer, Date and Datetime inputs retain exact physical endpoints.
/// All chunks form one collection. See [`intervals_core::cluster_intervals`].
///
/// # Errors
/// Rejects unequal lengths, nulls, unsupported or mismatched logical dtypes,
/// reversed intervals and IDs exceeding UInt32. Reports original row indices.
pub fn cluster_intervals(
    starts: &Series,
    ends: &Series,
    include_touching: bool,
) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::Cluster(include_touching))
}

fn coverage_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "max_k_coverage")?;
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

#[derive(serde::Deserialize)]
struct CoverageOptions {
    k: String,
}

#[pyo3_polars::derive::polars_expr(output_type_func = coverage_output)]
fn max_k_coverage_plugin(inputs: &[Series], kwargs: CoverageOptions) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "max_k_coverage")?;
    let k = kwargs
        .k
        .parse::<usize>()
        .map_err(|_| polars_err!(InvalidOperation: "k must be nonnegative and fit in usize"))?;
    max_k_coverage(starts, ends, k)
}

/// Select at most `k` intervals whose union has maximum total physical measure.
///
/// Among maximum-coverage solutions, use the fewest intervals. Returns a non-null
/// Boolean Series in original row order. Empty intervals are never selected.
/// Uses matching integer, Date, and Datetime logical dtypes, including timezone
/// metadata; exact physical days/ticks are widened before subtraction.
/// Each supplied collection is solved globally, across all chunks. See
/// [`intervals_core::max_k_coverage`] for the algorithm, complexity and ties.
///
/// # Errors
///
/// Rejects unequal lengths, null endpoints, mismatched/unsupported logical dtypes
/// and reversed intervals, preserving original row indices even when `k=0`.
pub fn max_k_coverage(starts: &Series, ends: &Series, k: usize) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::Coverage(k))
}

fn stabbing_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "minimum_stabbing_points")?;
    Ok(Field::new(
        start.name().clone(),
        DataType::List(Box::new(start.dtype().clone())),
    ))
}

#[pyo3_polars::derive::polars_expr(output_type_func = stabbing_output)]
fn minimum_stabbing_points_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "minimum_stabbing_points")?;
    minimum_stabbing_points(starts, ends)
}

/// Select the minimum number of discrete points hitting every half-open interval.
///
/// Returns one non-null list with the exact endpoint logical dtype, including
/// Date and Datetime unit/timezone metadata. Points are sorted and unique; empty
/// input returns one empty list. Date predecessors are days; Datetime predecessors
/// use the column's physical ms/us/ns tick. See [`intervals_core::minimum_stabbing_points`]
/// for the exchange proof, sorted-input fast path and `O(n log n)` time / `O(n)`
/// additional space bounds. Contiguous inputs are borrowed; multiple chunks are
/// collected before invoking the core.
///
/// # Errors
///
/// Rejects unequal lengths, null endpoints, unsupported or mismatched logical
/// dtypes, reversed intervals, and empty intervals. Interval errors preserve the
/// original zero-based row index within the supplied collection.
pub fn minimum_stabbing_points(starts: &Series, ends: &Series) -> PolarsResult<Series> {
    let points = evaluate(starts, ends, Algorithm::Stabbing)?;
    let points = logical_series(points, starts.dtype())?;
    Ok(Series::new("minimum_stabbing_points".into(), [points]))
}

fn input_pair<'a, T>(inputs: &'a [T], name: &str) -> PolarsResult<&'a [T; 2]> {
    inputs.try_into().map_err(|_| {
        polars_err!(InvalidOperation: "{} requires exactly two inputs, got {}",
            name, inputs.len())
    })
}

#[pyo3_polars::derive::polars_expr(output_type_func = count_output)]
fn overlap_count_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "overlap_count")?;
    overlap_count(starts, ends)
}

#[pyo3_polars::derive::polars_expr(output_type_func = containment_output)]
fn containment_count_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "containment_count")?;
    containment_count(starts, ends)
}

#[pyo3_polars::derive::polars_expr(output_type_func = nesting_output)]
fn nesting_depth_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "nesting_depth")?;
    nesting_depth(starts, ends)
}

#[pyo3_polars::derive::polars_expr(output_type_func = lanes_output)]
fn assign_lanes_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "assign_lanes")?;
    assign_lanes(starts, ends)
}

fn weighted_inputs<'a, T>(inputs: &'a [T], name: &str) -> PolarsResult<&'a [T; 3]> {
    inputs.try_into().map_err(|_| {
        polars_err!(InvalidOperation: "{} requires exactly three inputs, got {}", name, inputs.len())
    })
}

fn weighted_output(inputs: &[Field]) -> PolarsResult<Field> {
    weighted_field(inputs, "max_weight_non_overlapping")
}

fn capacity_output(inputs: &[Field]) -> PolarsResult<Field> {
    weighted_field(inputs, "max_weight_with_capacity")
}

fn optional_third<'a, T>(
    inputs: &'a [T],
    name: &str,
) -> PolarsResult<(&'a T, &'a T, Option<&'a T>)> {
    match inputs {
        [starts, ends] => Ok((starts, ends, None)),
        [starts, ends, weights] => Ok((starts, ends, Some(weights))),
        _ => polars_bail!(InvalidOperation:
            "{} requires two or three inputs, got {}", name, inputs.len()),
    }
}

fn clique_output(inputs: &[Field]) -> PolarsResult<Field> {
    let (start, end, weight) = optional_third(inputs, "max_weight_clique")?;
    validate_endpoint_dtypes(start.dtype(), end.dtype(), "max_weight_clique")?;
    if let Some(weight) = weight {
        validate_integer_dtype(weight.dtype(), "max_weight_clique", "weight")?;
    }
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

#[pyo3_polars::derive::polars_expr(output_type_func = clique_output)]
fn max_weight_clique_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let (starts, ends, weights) = optional_third(inputs, "max_weight_clique")?;
    max_weight_clique(starts, ends, weights)
}

fn weighted_field(inputs: &[Field], name: &str) -> PolarsResult<Field> {
    let [start, _, weight] = weighted_inputs(inputs, name)?;
    validate_integer_dtype(weight.dtype(), name, "weight")?;
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

fn is_supported_integer(dtype: &DataType) -> bool {
    matches!(
        dtype,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
    )
}

fn validate_integer_dtype(dtype: &DataType, name: &str, role: &str) -> PolarsResult<()> {
    polars_ensure!(is_supported_integer(dtype),
        InvalidOperation:
        "{} requires an 8-, 16-, 32-, or 64-bit integer {} dtype, got {}", name, role, dtype);
    Ok(())
}

#[pyo3_polars::derive::polars_expr(output_type_func = weighted_output)]
fn max_weight_non_overlapping_plugin(inputs: &[Series]) -> PolarsResult<Series> {
    let [starts, ends, weights] = weighted_inputs(inputs, "max_weight_non_overlapping")?;
    max_weight_non_overlapping(starts, ends, weights)
}

#[derive(serde::Deserialize)]
struct CapacityOptions {
    // serde-pickle only decodes signed 64-bit Python integers. Decimal text
    // transports the full usize range losslessly, including usize::MAX.
    capacity: String,
}

#[pyo3_polars::derive::polars_expr(output_type_func = capacity_output)]
fn max_weight_with_capacity_plugin(
    inputs: &[Series],
    kwargs: CapacityOptions,
) -> PolarsResult<Series> {
    let [starts, ends, weights] = weighted_inputs(inputs, "max_weight_with_capacity")?;
    let capacity = kwargs.capacity.parse::<usize>().map_err(
        |_| polars_err!(InvalidOperation: "capacity must be nonnegative and fit in usize"),
    )?;
    max_weight_with_capacity(starts, ends, weights, capacity)
}

/// Counts how many other intervals are contained by each row.
///
/// Containment means `a.start <= b.start && b.end <= a.end`, with self excluded.
/// Duplicate rows count each other. Half-open empty intervals follow exactly
/// the same endpoint predicate: `[0, 5)` contains `[5, 5)`.
///
/// Returns a non-null `UInt64` Series named `containment_count`, in original
/// row order across all chunks. Takes `O(n log n)` time and `O(n)` extra space.
///
/// # Errors
///
/// Rejects unequal lengths, null endpoints, and `start > end` (reporting the
/// first original row index). Logical dtypes must match exactly, including
/// Datetime unit and timezone. Supports 8/16/32/64-bit signed/unsigned integers,
/// Date, and Datetime; physical integers are extracted only after validation.
/// No coercion or scalar broadcasting is performed.
///
/// # Examples
///
/// ```
/// use polars::prelude::*;
/// let starts = Series::new("start".into(), [0i64, 2, 4]);
/// let ends = Series::new("end".into(), [10i64, 5, 12]);
/// let counts = polars_intervals::containment_count(&starts, &ends)?;
/// assert_eq!(counts.u64()?.into_no_null_iter().collect::<Vec<_>>(), [1, 0, 0]);
/// # Ok::<(), PolarsError>(())
/// ```
pub fn containment_count(starts: &Series, ends: &Series) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::ContainmentCount)
}

/// Return the length of the longest strict containment chain above each interval.
///
/// Outermost intervals have depth zero. `A` strictly contains `B` exactly when
/// `A.start <= B.start && B.end <= A.end && (A.start < B.start || B.end < A.end)`.
/// Identical geometries share a depth and never add a level to one another;
/// equal starts or equal ends alone still permit strict containment. Empty
/// intervals follow this endpoint predicate: `[0, 5)` strictly contains `[5, 5)`,
/// while identical `[5, 5)` rows do not strictly contain one another.
///
/// Returns a non-null `UInt64` Series named `nesting_depth`, in original row
/// order. All chunks form one collection; each Python window/group is solved
/// independently. Unlike [`containment_count`], this measures a longest chain,
/// not the number of contained rows or containers. Several incomparable
/// containers can all contain one interval whose depth is only one.
/// See [`intervals_core::nesting_depths`] for the production algorithm; worst-case
/// time is `O(n log n)` and additional space is `O(n)`.
///
/// # Errors
///
/// Rejects unequal lengths, null endpoints, unsupported or mismatched logical
/// dtypes, and `start > end`, reporting the first original row index. Supports
/// 8/16/32/64-bit signed/unsigned integers, Date, and Datetime with exactly
/// matching units/timezones, using physical endpoints without coercion or
/// scalar broadcasting. Empty input returns empty output.
///
/// # Examples
///
/// ```
/// use polars::prelude::*;
/// let starts = Series::new("start".into(), [0i64, 2, 2, 3]);
/// let ends = Series::new("end".into(), [10i64, 8, 8, 7]);
/// let depths = polars_intervals::nesting_depth(&starts, &ends)?;
/// assert_eq!(depths.u64()?.into_no_null_iter().collect::<Vec<_>>(), [0, 1, 1, 2]);
/// # Ok::<(), PolarsError>(())
/// ```
pub fn nesting_depth(starts: &Series, ends: &Series) -> PolarsResult<Series> {
    evaluate(starts, ends, Algorithm::NestingDepth)
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

fn balanced_lanes_output(inputs: &[Field]) -> PolarsResult<Field> {
    let (start, _, initial_lanes) = optional_third(inputs, "assign_balanced_lanes")?;
    if let Some(lanes) = initial_lanes {
        validate_integer_dtype(lanes.dtype(), "assign_balanced_lanes", "lane")?;
    }
    Ok(Field::new(start.name().clone(), DataType::UInt32))
}

#[derive(serde::Deserialize)]
struct BalanceOptions {
    // Decimal text transports the entire u64 range through serde-pickle.
    max_work: String,
}

impl BalanceOptions {
    fn max_work(&self) -> PolarsResult<u64> {
        self.max_work.parse().map_err(
            |_| polars_err!(InvalidOperation: "max_work must be nonnegative and fit in UInt64"),
        )
    }
}

#[pyo3_polars::derive::polars_expr(output_type_func = balanced_lanes_output)]
fn assign_balanced_lanes_plugin(inputs: &[Series], kwargs: BalanceOptions) -> PolarsResult<Series> {
    let (starts, ends, initial_lanes) = optional_third(inputs, "assign_balanced_lanes")?;
    assign_balanced_lanes(starts, ends, initial_lanes, kwargs.max_work()?)
}

/// Assign the minimum number of lanes while heuristically balancing row counts.
///
/// Returns non-null contiguous `UInt32` labels in original row order. Minimize
/// `(max(size) - min(size), sum(size * size))` lexicographically using exact
/// integers, never occupied duration. Without `initial_lanes`, starts from
/// [`assign_lanes`]; otherwise starts from the supplied proper minimum-lane
/// assignment. The result preserves the palette and never worsens its starting
/// score, but individual rows may change lanes. A minimum lane count and proper
/// coloring are guaranteed; globally optimal balance or an approximation ratio
/// is not. Every empty row counts and
/// may use any existing lane, unlike [`assign_lanes`], which uses lane zero.
/// Nonempty empty-only input uses one lane; empty input uses zero lanes.
///
/// `max_work` bounds deterministic refinement work, not elapsed time; zero
/// returns the validated starting assignment unchanged. Validation and fixed
/// `O(n log n)` preparation are outside that budget; bounded refinement adds
/// work beyond construction. See [`intervals_core::assign_balanced_lanes`] for
/// work units and the algorithm. All chunks form one collection, including in
/// streaming queries; Python window/group expressions solve each group whole.
///
/// # Errors
///
/// Endpoint validation matches [`assign_lanes`]: matching integer, Date, or
/// Datetime dtypes including units/timezones, no nulls or broadcasting, and
/// original row indices for reversed intervals. Identical inputs/options are
/// deterministic; labels and heuristic quality need not survive row permutation.
///
/// In addition to endpoint errors, rejects unequal lane lengths, null lane IDs,
/// sparse/noncontiguous labels, conflicts, or a nonminimum palette. Lane columns
/// must be 8/16/32/64-bit signed/unsigned integers, with every value nonnegative
/// and in the UInt32 range; no lossy cast is performed. Boolean, floating-point,
/// temporal, and wider integer lane dtypes are unsupported, even on empty input.
/// All validation precedes zero-budget or balance early returns.
pub fn assign_balanced_lanes(
    starts: &Series,
    ends: &Series,
    initial_lanes: Option<&Series>,
    max_work: u64,
) -> PolarsResult<Series> {
    let lanes = initial_lanes
        .map(|lanes| {
            validate_integer_dtype(lanes.dtype(), "assign_balanced_lanes", "lane")?;
            polars_ensure!(starts.len() == lanes.len(), ShapeMismatch:
                "assign_balanced_lanes requires equal lengths, got {} intervals and {} lanes",
                starts.len(), lanes.len());
            polars_ensure!(lanes.null_count() == 0, ComputeError:
                "assign_balanced_lanes does not support null lane IDs");
            integer_values(lanes)?
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    u32::try_from(value).map_err(|_| {
                        polars_err!(InvalidOperation:
                            "assign_balanced_lanes requires nonnegative UInt32-range lane IDs; invalid value at index {}",
                            index)
                    })
                })
                .collect::<PolarsResult<Vec<_>>>()
            })
        .transpose()?;
    evaluate(
        starts,
        ends,
        Algorithm::AssignBalancedLanes(lanes.as_deref(), max_work),
    )
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
    evaluate_weighted(starts, ends, weights, None)
}

/// Select one globally maximum-weight clique of the interval intersection graph.
///
/// Returns a non-null Boolean Series in original row order. `None` assigns every
/// row unit weight without constructing a weight vector. Explicit weights must
/// be signed/unsigned integers up to 64 bits; nonpositive rows are omitted.
/// Half-open nonempty intervals are adjacent exactly when they overlap; touching
/// intervals are not adjacent. Each empty interval is an isolated vertex, hence
/// may only be selected alone. The empty clique has weight zero.
///
/// Matching integer, Date and Datetime endpoints follow [`overlap_count`]. All
/// chunks form one instance. Ties choose the earliest maximizing coordinate,
/// prefer a nonempty clique over an equal-weight empty singleton, then choose
/// the lowest original row among tied empty singletons. Tied masks are not a
/// stability promise across releases. See [`intervals_core::max_weight_clique`]
/// for the exact algorithm and its `O(n log n)` time / `O(n)` space bounds.
///
/// # Errors
///
/// Rejects unequal lengths, nulls, unsupported or mismatched logical dtypes,
/// reversed intervals (with original row index), and checked i128 objective
/// overflow. Every row is validated, including those with nonpositive weights.
/// No implicit casting, null filling, or scalar broadcasting occurs.
pub fn max_weight_clique(
    starts: &Series,
    ends: &Series,
    weights: Option<&Series>,
) -> PolarsResult<Series> {
    let Some(weights) = weights else {
        return evaluate(starts, ends, Algorithm::Clique(None));
    };
    validate_integer_dtype(weights.dtype(), "max_weight_clique", "weight")?;
    polars_ensure!(starts.len() == weights.len(), ShapeMismatch:
        "max_weight_clique requires equal lengths, got {} intervals and {} weights",
        starts.len(), weights.len());
    polars_ensure!(weights.null_count() == 0, ComputeError:
        "max_weight_clique does not support null weights");
    let weights = integer_values(weights)?;
    evaluate(starts, ends, Algorithm::Clique(Some(&weights)))
}

/// Select a globally maximum-weight subset subject to maximum simultaneous capacity.
///
/// Returns a non-null Boolean Series named `max_weight_with_capacity` in original
/// row order. Matching integer, Date, and Datetime endpoints follow
/// [`overlap_count`]. Weights follow [`max_weight_non_overlapping`]: non-null
/// signed/unsigned integers up to 64 bits, accumulated exactly in checked i128.
/// Positive empty intervals consume no capacity and are always selected;
/// nonpositive weights are omitted. Ties are deterministic but unspecified.
/// Capacity zero permits only empty intervals; one delegates to the specialized
/// weighted scheduler. Each collection (including all chunks) is one instance.
///
/// The core uses exact min-cost flow with sufficient-capacity and overlap-component
/// fast paths: `O(n log n + sum(k * n_c * log(n_c + 1)))` time over constrained
/// components, `O(n)` additional space. See [`intervals_core::max_weight_with_capacity`].
///
/// # Errors
///
/// Rejects unequal lengths, nulls, unsupported weights/endpoints, mismatched
/// endpoint logical dtypes, reversed intervals with original row index, and
/// i128 overflow. No dtype coercion or scalar broadcasting is performed.
pub fn max_weight_with_capacity(
    starts: &Series,
    ends: &Series,
    weights: &Series,
    capacity: usize,
) -> PolarsResult<Series> {
    evaluate_weighted(starts, ends, weights, Some(capacity))
}

fn evaluate_weighted(
    starts: &Series,
    ends: &Series,
    weights: &Series,
    capacity: Option<usize>,
) -> PolarsResult<Series> {
    let name = if capacity.is_some() {
        "max_weight_with_capacity"
    } else {
        "max_weight_non_overlapping"
    };
    validate_integer_dtype(weights.dtype(), name, "weight")?;
    polars_ensure!(starts.len() == weights.len(), ShapeMismatch:
        "{} requires equal lengths, got {} intervals and {} weights",
        name, starts.len(), weights.len());
    polars_ensure!(weights.null_count() == 0, ComputeError:
        "{} does not support null weights", name);
    let weights = integer_values(weights)?;
    let algorithm = match capacity {
        Some(capacity) => Algorithm::Capacity(&weights, capacity),
        None => Algorithm::MaxWeight(&weights),
    };
    evaluate(starts, ends, algorithm)
}

fn integer_values(values: &Series) -> PolarsResult<Vec<i128>> {
    // Normalize only the accumulator representation, avoiding 8 x 8 endpoint /
    // weight/cost monomorphizations. No Polars cast or loss of integer precision.
    macro_rules! widen {
        ($accessor:ident) => {
            values
                .$accessor()?
                .into_no_null_iter()
                .map(i128::from)
                .collect::<Vec<_>>()
        };
    }
    Ok(match values.dtype() {
        DataType::Int8 => widen!(i8),
        DataType::Int16 => widen!(i16),
        DataType::Int32 => widen!(i32),
        DataType::Int64 => widen!(i64),
        DataType::UInt8 => widen!(u8),
        DataType::UInt16 => widen!(u16),
        DataType::UInt32 => widen!(u32),
        DataType::UInt64 => widen!(u64),
        _ => unreachable!("integer dtype was validated by the caller"),
    })
}

#[derive(Clone, Copy)]
enum Algorithm<'a> {
    Cluster(bool),
    Coverage(usize),
    Stabbing,
    OverlapCount,
    ContainmentCount,
    NestingDepth,
    AssignLanes,
    AssignBalancedLanes(Option<&'a [u32]>, u64),
    MaxWeight(&'a [i128]),
    Clique(Option<&'a [i128]>),
    DominatingSet(Option<&'a [i128]>),
    Capacity(&'a [i128], usize),
    Cover(i128, i128),
    CostCover(&'a [i128], i128, i128),
}

impl Algorithm<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::Cluster(_) => "cluster_intervals",
            Self::Coverage(_) => "max_k_coverage",
            Self::Stabbing => "minimum_stabbing_points",
            Self::OverlapCount => "overlap_count",
            Self::ContainmentCount => "containment_count",
            Self::NestingDepth => "nesting_depth",
            Self::AssignLanes => "assign_lanes",
            Self::AssignBalancedLanes(_, _) => "assign_balanced_lanes",
            Self::MaxWeight(_) => "max_weight_non_overlapping",
            Self::Clique(_) => "max_weight_clique",
            Self::DominatingSet(_) => "minimum_cost_dominating_set",
            Self::Capacity(_, _) => "max_weight_with_capacity",
            Self::Cover(_, _) => "minimum_cover",
            Self::CostCover(_, _, _) => "minimum_cost_cover",
        }
    }
}

fn validate_endpoint_pair(starts: &Series, ends: &Series, name: &str) -> PolarsResult<()> {
    polars_ensure!(starts.len() == ends.len(), ShapeMismatch:
        "{} requires equal lengths, got {} starts and {} ends", name, starts.len(), ends.len());
    validate_endpoint_dtypes(starts.dtype(), ends.dtype(), name)
}

fn validate_endpoint_dtypes(starts: &DataType, ends: &DataType, name: &str) -> PolarsResult<()> {
    polars_ensure!(starts == ends, InvalidOperation:
        "{} requires matching integer, Date, or Datetime dtypes (including Datetime time unit and timezone), got {} and {}",
        name, starts, ends);
    polars_ensure!(is_supported_integer(starts) || matches!(starts,
        DataType::Date | DataType::Datetime(_, _)), InvalidOperation:
        "{} requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}",
        name, starts);
    Ok(())
}

fn endpoint_values<'a, T: PolarsIntegerType>(
    starts: &'a ChunkedArray<T>,
    ends: &'a ChunkedArray<T>,
    name: &str,
) -> PolarsResult<[Cow<'a, [T::Native]>; 2]> {
    if starts.null_count() != 0 || ends.null_count() != 0 {
        let index = starts
            .iter()
            .zip(ends.iter())
            .position(|(start, end)| start.is_none() || end.is_none())
            .expect("a null endpoint exists");
        polars_bail!(ComputeError: "{} does not support null endpoints at index {}", name, index);
    }
    Ok(endpoint_slices([starts, ends]))
}

fn endpoint_slices<'a, T: PolarsIntegerType, const N: usize>(
    columns: [&'a ChunkedArray<T>; N],
) -> [Cow<'a, [T::Native]>; N] {
    // Borrow contiguous inputs; collect only columns spanning multiple chunks.
    columns.map(|column| {
        column
            .cont_slice()
            .map(Cow::Borrowed)
            .unwrap_or_else(|_| Cow::Owned(column.into_no_null_iter().collect()))
    })
}

fn native_domain<T: TryFrom<i128>>(domain: Option<(i128, i128)>) -> PolarsResult<Option<(T, T)>> {
    domain
        .map(|(left, right)| {
            let convert = |value| {
                T::try_from(value).map_err(|_| {
                    polars_err!(InvalidOperation: "domain bound is outside endpoint dtype range")
                })
            };
            Ok((convert(left)?, convert(right)?))
        })
        .transpose()
}

fn logical_series(physical: Series, dtype: &DataType) -> PolarsResult<Series> {
    match dtype {
        // Integer-to-Datetime casts without the optional timezone feature discard
        // the zone. Restore metadata directly; no time conversion is needed.
        DataType::Datetime(unit, zone) => Ok(physical.into_datetime(*unit, zone.clone())),
        dtype => physical.cast(dtype),
    }
}

fn validate_group_keys(
    keys: &[Series],
    len: usize,
    name: &str,
    reserved: &[&str],
) -> PolarsResult<()> {
    for (index, key) in keys.iter().enumerate() {
        polars_ensure!(key.len() == len, ShapeMismatch:
            "{} requires equal interval and group key lengths", name);
        polars_ensure!(!reserved.contains(&key.name().as_str()), InvalidOperation:
            "{} group keys cannot use reserved output names {}", name, reserved.join("/"));
        polars_ensure!(!keys[..index].iter().any(|other| other.name() == key.name()), InvalidOperation:
            "{} requires distinct group keys", name);
        polars_ensure!(is_supported_integer(key.dtype()) || matches!(key.dtype(),
            DataType::String | DataType::Boolean |
            DataType::Date | DataType::Datetime(_, _)), InvalidOperation:
            "{} requires String, Boolean, 8/16/32/64-bit integer, Date, or Datetime group keys, got {}",
            name, key.dtype());
    }
    Ok(())
}

fn evaluate(starts: &Series, ends: &Series, algorithm: Algorithm<'_>) -> PolarsResult<Series> {
    validate_endpoint_pair(starts, ends, algorithm.name())?;
    dispatch_endpoints!(starts.dtype(), algorithm.name(), |cast| evaluate_typed(
        cast(starts)?,
        cast(ends)?,
        algorithm
    ))
}

fn evaluate_typed<T>(
    starts: &ChunkedArray<T>,
    ends: &ChunkedArray<T>,
    algorithm: Algorithm<'_>,
) -> PolarsResult<Series>
where
    T: PolarsIntegerType,
    T::Native: intervals_core::DiscreteEndpoint + intervals_core::CoverageEndpoint + TryFrom<i128>,
{
    let [starts, ends] = endpoint_values(starts, ends, algorithm.name())?;
    match algorithm {
        Algorithm::Cluster(include_touching) => {
            let labels = intervals_core::cluster_intervals(&starts, &ends, include_touching)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::from_vec(algorithm.name().into(), labels))
        }
        Algorithm::Coverage(k) => {
            let mask = intervals_core::max_k_coverage(&starts, &ends, k)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
        Algorithm::Stabbing => {
            let points = intervals_core::minimum_stabbing_points(&starts, &ends)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(ChunkedArray::<T>::from_vec(algorithm.name().into(), points).into_series())
        }
        Algorithm::OverlapCount | Algorithm::ContainmentCount | Algorithm::NestingDepth => {
            let counts = match algorithm {
                Algorithm::ContainmentCount => intervals_core::containment_counts(&starts, &ends),
                Algorithm::NestingDepth => intervals_core::nesting_depths(&starts, &ends),
                _ => intervals_core::overlap_counts(&starts, &ends),
            }
            .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            let counts: Vec<u64> = counts.into_iter().map(|count| count as u64).collect();
            Ok(Series::from_vec(algorithm.name().into(), counts))
        }
        Algorithm::AssignLanes => {
            let lanes = intervals_core::assign_lanes(&starts, &ends)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::from_vec(algorithm.name().into(), lanes))
        }
        Algorithm::AssignBalancedLanes(initial_lanes, max_work) => {
            let lanes =
                intervals_core::assign_balanced_lanes(&starts, &ends, initial_lanes, max_work)
                    .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::from_vec(algorithm.name().into(), lanes))
        }
        Algorithm::MaxWeight(weights) => {
            let mask = intervals_core::max_weight_non_overlapping(&starts, &ends, weights)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
        Algorithm::Clique(weights) => {
            let mask = match weights {
                Some(weights) => intervals_core::max_weight_clique(&starts, &ends, weights),
                None => intervals_core::max_clique(&starts, &ends),
            }
            .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
        Algorithm::DominatingSet(costs) => {
            let mask = match costs {
                Some(costs) => intervals_core::minimum_cost_dominating_set(&starts, &ends, costs),
                None => intervals_core::minimum_dominating_set(&starts, &ends),
            }
            .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
        Algorithm::Capacity(weights, capacity) => {
            let mask = intervals_core::max_weight_with_capacity(&starts, &ends, weights, capacity)
                .map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
        Algorithm::Cover(left, right) | Algorithm::CostCover(_, left, right) => {
            let convert = |value| {
                T::Native::try_from(value).map_err(|_|
                polars_err!(InvalidOperation: "target value is outside the endpoint dtype range"))
            };
            let (left, right) = (convert(left)?, convert(right)?);
            let result = match algorithm {
                Algorithm::CostCover(costs, _, _) => {
                    intervals_core::minimum_cost_cover(&starts, &ends, costs, left, right)
                }
                _ => intervals_core::minimum_cover(&starts, &ends, left, right),
            };
            let mask = result.map_err(|error| polars_err!(ComputeError: "{error}"))?;
            Ok(Series::new(algorithm.name().into(), mask))
        }
    }
}
