use super::{
    Algorithm, evaluate, integer_values, optional_third, validate_endpoint_dtypes,
    validate_integer_dtype,
};
use polars::prelude::*;

// Validate fields through output_type_func: unsupported imported dtypes must
// return a Polars error rather than panic across the expression FFI boundary.
fn output(fields: &[Field]) -> PolarsResult<Field> {
    let (start, end, cost) = optional_third(fields, "minimum_cost_dominating_set")?;
    validate_endpoint_dtypes(start.dtype(), end.dtype(), "minimum_cost_dominating_set")?;
    if let Some(cost) = cost {
        validate_integer_dtype(cost.dtype(), "minimum_cost_dominating_set", "cost")?;
    }
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

#[pyo3_polars::derive::polars_expr(output_type_func = output)]
fn minimum_cost_dominating_set_plugin(values: &[Series]) -> PolarsResult<Series> {
    let (starts, ends, costs) = optional_third(values, "minimum_cost_dominating_set")?;
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
