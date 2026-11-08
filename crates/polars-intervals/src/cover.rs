use super::{
    Algorithm, evaluate, input_pair, integer_values, validate_integer_dtype, weighted_inputs,
};
use polars::prelude::*;

#[derive(serde::Deserialize, pyo3::FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct TargetValue {
    kind: String,
    // Decimal text avoids serde-pickle's signed-64-bit integer limit.
    value: String,
    #[pyo3(default)]
    dtype: Option<String>,
    #[pyo3(default)]
    unit: Option<String>,
    #[pyo3(default)]
    timezone: Option<String>,
}

impl TargetValue {
    pub(crate) fn physical(&self, dtype: &DataType) -> PolarsResult<i128> {
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

pub(crate) fn physical_domain(
    domain: Option<(TargetValue, TargetValue)>,
    dtype: &DataType,
) -> PolarsResult<Option<(i128, i128)>> {
    domain
        .map(|(left, right)| Ok((left.physical(dtype)?, right.physical(dtype)?)))
        .transpose()
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
