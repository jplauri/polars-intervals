use super::{
    Algorithm, evaluate, input_pair, integer_values, validate_integer_dtype, weighted_inputs,
};
use polars::prelude::*;

#[derive(serde::Deserialize)]
struct TargetValue {
    kind: String,
    // Decimal text avoids serde-pickle's signed-64-bit integer limit.
    value: String,
    dtype: Option<String>,
    unit: Option<String>,
    timezone: Option<String>,
}

impl TargetValue {
    fn physical(&self, dtype: &DataType) -> PolarsResult<i128> {
        let compatible = match (self.kind.as_str(), dtype) {
            ("integer", dtype) if dtype.is_integer() => self
                .dtype
                .as_ref()
                .is_none_or(|d| *d == format!("{dtype:?}")),
            ("date", DataType::Date) => true,
            ("datetime", DataType::Datetime(unit, timezone)) => {
                self.unit.as_deref() == Some(unit.to_ascii())
                    && self.timezone.as_deref() == timezone.as_ref().map(|tz| tz.as_str())
            }
            _ => false,
        };
        polars_ensure!(compatible, InvalidOperation:
            "target scalar must exactly match endpoint dtype {} (including Datetime unit and timezone)", dtype);
        self.value.parse().map_err(
            |_| polars_err!(InvalidOperation: "target integer is outside the supported range"),
        )
    }
}

#[derive(serde::Deserialize)]
struct CoverOptions {
    target_start: TargetValue,
    target_end: TargetValue,
}

fn cover_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _] = input_pair(inputs, "minimum_cover")?;
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

fn cost_cover_output(inputs: &[Field]) -> PolarsResult<Field> {
    let [start, _, cost] = weighted_inputs(inputs, "minimum_cost_cover")?;
    validate_integer_dtype(cost.dtype(), "minimum_cost_cover", "cost")?;
    Ok(Field::new(start.name().clone(), DataType::Boolean))
}

#[pyo3_polars::derive::polars_expr(output_type_func = cover_output)]
fn minimum_cover_plugin(inputs: &[Series], kwargs: CoverOptions) -> PolarsResult<Series> {
    let [starts, ends] = input_pair(inputs, "minimum_cover")?;
    let left = kwargs.target_start.physical(starts.dtype())?;
    let right = kwargs.target_end.physical(starts.dtype())?;
    evaluate(starts, ends, Algorithm::Cover(left, right))
}

#[pyo3_polars::derive::polars_expr(output_type_func = cost_cover_output)]
fn minimum_cost_cover_plugin(inputs: &[Series], kwargs: CoverOptions) -> PolarsResult<Series> {
    let [starts, ends, costs] = weighted_inputs(inputs, "minimum_cost_cover")?;
    let left = kwargs.target_start.physical(starts.dtype())?;
    let right = kwargs.target_end.physical(starts.dtype())?;
    evaluate_cost_cover(starts, ends, costs, left, right)
}

fn scalar_physical(target: &Scalar, dtype: &DataType) -> PolarsResult<i128> {
    polars_ensure!(target.dtype() == dtype, InvalidOperation:
        "target scalar must exactly match endpoint dtype {} (including Datetime unit and timezone)", dtype);
    match target.as_any_value().to_physical() {
        AnyValue::Int8(v) => Ok(v.into()),
        AnyValue::Int16(v) => Ok(v.into()),
        AnyValue::Int32(v) => Ok(v.into()),
        AnyValue::Int64(v) => Ok(v.into()),
        AnyValue::UInt8(v) => Ok(v.into()),
        AnyValue::UInt16(v) => Ok(v.into()),
        AnyValue::UInt32(v) => Ok(v.into()),
        AnyValue::UInt64(v) => Ok(v.into()),
        _ => {
            polars_bail!(InvalidOperation: "target must be a non-null supported integer, Date or Datetime scalar")
        }
    }
}

/// Select the fewest intervals continuously covering the half-open scalar target.
///
/// Target [`Scalar`] dtypes must exactly match the endpoint Series, including
/// Datetime units and timezone metadata. Endpoint rules match [`super::overlap_count`].
/// Returns a non-null Boolean Series in original row order. All chunks form
/// one instance. Empty targets select nothing; empty intervals are never selected.
/// Uses exact furthest-reaching greedy selection in `O(n log n)` time / `O(n)` space.
/// Ties are deterministic; a particular optimal mask is not a stable guarantee.
///
/// # Errors
/// Rejects mismatched dtypes/lengths, nulls, reversed intervals/targets, out-of-range
/// targets and infeasible coverage. There is no casting or broadcasting.
pub fn minimum_cover(
    starts: &Series,
    ends: &Series,
    target_start: &Scalar,
    target_end: &Scalar,
) -> PolarsResult<Series> {
    let left = scalar_physical(target_start, starts.dtype())?;
    let right = scalar_physical(target_end, starts.dtype())?;
    evaluate(starts, ends, Algorithm::Cover(left, right))
}

/// Select a minimum-cost continuous cover, breaking cost ties by fewer intervals.
///
/// Target and endpoint semantics match [`minimum_cover`]. Costs must be non-null,
/// nonnegative Int8/16/32/64 or UInt8/16/32/64, accumulated exactly with checked
/// `i128` arithmetic. Uses frontier DP with a Fenwick suffix-min tree in
/// `O(n log n)` time and `O(n)` additional space, including reconstruction.
///
/// # Errors
/// Besides [`minimum_cover`]'s errors, rejects unsupported/null/negative costs,
/// cost length mismatch, and an optimal total exceeding `i128::MAX`.
pub fn minimum_cost_cover(
    starts: &Series,
    ends: &Series,
    costs: &Series,
    target_start: &Scalar,
    target_end: &Scalar,
) -> PolarsResult<Series> {
    let left = scalar_physical(target_start, starts.dtype())?;
    let right = scalar_physical(target_end, starts.dtype())?;
    evaluate_cost_cover(starts, ends, costs, left, right)
}

fn evaluate_cost_cover(
    starts: &Series,
    ends: &Series,
    costs: &Series,
    left: i128,
    right: i128,
) -> PolarsResult<Series> {
    validate_integer_dtype(costs.dtype(), "minimum_cost_cover", "cost")?;
    polars_ensure!(starts.len() == costs.len(), ShapeMismatch: "minimum_cost_cover requires equal interval and cost lengths");
    polars_ensure!(costs.null_count() == 0, ComputeError: "minimum_cost_cover does not support null costs");
    let costs = integer_values(costs)?;
    evaluate(starts, ends, Algorithm::CostCover(&costs, left, right))
}
