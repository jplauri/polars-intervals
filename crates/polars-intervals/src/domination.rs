use super::{Algorithm, evaluate, integer_values, validate_integer_dtype};
use polars::prelude::*;

fn domination_inputs<T>(values: &[T]) -> PolarsResult<(&T, &T, Option<&T>)> {
    match values {
        [starts, ends] => Ok((starts, ends, None)),
        [starts, ends, costs] => Ok((starts, ends, Some(costs))),
        _ => polars_bail!(InvalidOperation:
            "minimum_cost_dominating_set requires two or three inputs, got {}", values.len()),
    }
}

// Validate fields through output_type_func: unsupported imported dtypes must
// return a Polars error rather than panic across the expression FFI boundary.
fn output(fields: &[Field]) -> PolarsResult<Field> {
    let (start, end, cost) = domination_inputs(fields)?;
    polars_ensure!(start.dtype() == end.dtype(), InvalidOperation:
        "minimum_cost_dominating_set requires matching integer, Date, or Datetime dtypes (including Datetime time unit and timezone), got {} and {}",
        start.dtype(), end.dtype());
    polars_ensure!(matches!(start.dtype(),
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 |
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 |
        DataType::Date | DataType::Datetime(_, _)), InvalidOperation:
        "minimum_cost_dominating_set requires an 8-, 16-, 32-, or 64-bit integer dtype, Date, or Datetime, got {}",
        start.dtype());
    if let Some(cost) = cost {
        validate_integer_dtype(cost.dtype(), "minimum_cost_dominating_set", "cost")?;
    }
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

#[pyo3_polars::derive::polars_expr(output_type_func = output)]
fn minimum_cost_dominating_set_plugin(values: &[Series]) -> PolarsResult<Series> {
    let (starts, ends, costs) = domination_inputs(values)?;
    minimum_cost_dominating_set(starts, ends, costs)
}

/// Select an exact minimum-cost dominating set, breaking cost ties by fewer rows.
///
/// Every row must be selected or overlap a selected nonempty row. Intervals are
/// half-open; touching intervals do not overlap. Every empty row is isolated and
/// must be selected, even with zero cost or duplicate coordinates. Selected rows
/// dominate themselves, so a nonempty collection never selects nothing.
///
/// `None` assigns unit costs without constructing a cost vector. Explicit costs
/// must be nonnegative Int8/16/32/64 or UInt8/16/32/64. Costs use exact checked
/// `i128` arithmetic. Returns a non-null Boolean Series in original row order.
/// Matching integer, Date and Datetime endpoints follow [`super::overlap_count`].
/// All chunks form one instance. Filtering beforehand changes both candidates
/// and vertices requiring domination. Identical inputs are deterministic; no
/// particular tied optimum is promised across releases. The core takes
/// `O(n log n)` time and `O(n)` extra space, including reduction and reconstruction.
/// See [`intervals_core::minimum_cost_dominating_set`] for the exact algorithm.
///
/// # Errors
///
/// Rejects unequal lengths, nulls, unsupported or mismatched logical dtypes,
/// negative costs, reversed intervals (with original row index), and an optimal
/// total exceeding `i128::MAX`. Every row is validated before optimization.
/// No implicit casting, null filling, or scalar broadcasting occurs.
pub fn minimum_cost_dominating_set(
    starts: &Series,
    ends: &Series,
    costs: Option<&Series>,
) -> PolarsResult<Series> {
    let Some(costs) = costs else {
        return evaluate(starts, ends, Algorithm::DominatingSet(None));
    };
    validate_integer_dtype(costs.dtype(), "minimum_cost_dominating_set", "cost")?;
    polars_ensure!(starts.len() == costs.len(), ShapeMismatch:
        "minimum_cost_dominating_set requires equal lengths, got {} intervals and {} costs",
        starts.len(), costs.len());
    polars_ensure!(costs.null_count() == 0, ComputeError:
        "minimum_cost_dominating_set does not support null costs");
    let costs = integer_values(costs)?;
    evaluate(starts, ends, Algorithm::DominatingSet(Some(&costs)))
}
