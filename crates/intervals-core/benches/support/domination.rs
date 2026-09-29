//! Complete-call private candidates, shared by domination tests and benchmarks.
#![allow(dead_code)]
use intervals_core::IntervalError;

#[path = "../../src/cover.rs"]
mod cover;
#[path = "../../src/domination.rs"]
mod production;

pub(crate) use production::{Reduction, reduce};
pub const METHODS: &[&str] = &["reduction", "fused", "heap", "greedy", "quadratic"];

// Source-included modules cannot access the library's private validator.
fn validate_lengths<T>(starts: &[T], ends: &[T]) -> Result<(), IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch([
            ("starts", starts.len()),
            ("ends", ends.len()),
        ]));
    }
    Ok(())
}

fn value<W: Copy>(costs: Option<&[W]>, row: usize) -> i128
where
    i128: From<W>,
{
    costs.map_or(1, |costs| i128::from(costs[row]))
}

pub fn run<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    costs: Option<&[W]>,
    method: &str,
) -> Result<Vec<bool>, IntervalError>
where
    i128: From<W>,
{
    if method == "reduction" {
        return reduction_cover(starts, ends, costs);
    }
    validate_lengths(starts, ends)?;
    if let Some(costs) = costs {
        cover::validate_costs(costs, starts.len())?;
    }
    if method == "greedy" {
        // A common nonnegative cost minimizes count, including common zero.
        // Heterogeneous costs require one of the weighted methods.
        assert!(costs.is_none_or(|w| w.iter().all(|&c| i128::from(c) == i128::from(w[0]))));
        let mask = production::minimum_dominating_set(starts, ends)?;
        if let Some(costs) = costs {
            production::check_selected_cost(&mask, costs)?;
        }
        return Ok(mask);
    }
    let mut reduction = reduce(starts, ends)?;
    let mut mask = vec![false; starts.len()];
    for &row in &reduction.empties {
        mask[row] = true;
    }
    if !reduction.targets.is_empty() {
        match method {
            "fused" => match costs {
                None => {
                    // Extraction already supplied blocks ordered by start.
                    cover::greedy(&reduction.blocks, 0, reduction.targets.len(), &mut mask)?;
                }
                Some(costs) => {
                    // Reuse the cover DP without auxiliary endpoint arrays,
                    // clipping, repeated validation, or a copied candidate list.
                    reduction.blocks.sort_unstable_by_key(|c| (c.end, c.row));
                    let coordinates = cover::coordinates(&reduction.blocks, 0);
                    let solution = cover::dynamic_program(&reduction.blocks, &coordinates, costs);
                    if !cover::reconstruct(&solution.back, &mut mask) {
                        // Every domination instance is geometrically feasible.
                        return Err(IntervalError::CostOverflow);
                    }
                }
            },
            "heap" => production::prefix_heap(&reduction, costs, &mut mask)?,
            "quadratic" => prefix_quadratic(&reduction, costs, &mut mask)?,
            _ => panic!("unknown domination candidate: {method}"),
        }
    }
    if let Some(costs) = costs {
        production::check_selected_cost(&mask, costs)?;
    }
    Ok(mask)
}

/// Retain baseline A independently of whichever route production selects.
fn reduction_cover<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    costs: Option<&[W]>,
) -> Result<Vec<bool>, IntervalError>
where
    i128: From<W>,
{
    validate_lengths(starts, ends)?;
    if let Some(costs) = costs {
        cover::validate_costs(costs, starts.len())?;
    }
    let reduction = reduce(starts, ends)?;
    let mut left = vec![0; starts.len()];
    let mut right = vec![0; starts.len()];
    for block in &reduction.blocks {
        left[block.row] = block.start;
        right[block.row] = block.end;
    }
    let mut mask = match costs {
        None => cover::minimum_cover(&left, &right, 0, reduction.targets.len())?,
        Some(costs) => cover::minimum_cost_cover(&left, &right, costs, 0, reduction.targets.len())?,
    };
    for row in reduction.empties {
        mask[row] = true;
    }
    if let Some(costs) = costs {
        production::check_selected_cost(&mask, costs)?;
    }
    Ok(mask)
}

fn prefix_quadratic<T, W: Copy>(
    reduction: &Reduction<T>,
    costs: Option<&[W]>,
    mask: &mut [bool],
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let len = reduction.targets.len();
    let mut scores = vec![None; len + 1];
    scores[0] = Some((0i128, 0usize));
    let mut back = vec![None; len + 1];
    for j in 0..len {
        let mut best = None;
        for c in &reduction.blocks {
            if c.start <= j
                && j < c.end
                && let Some((cost, count)) = scores[c.start]
                && let Some(cost) = cost.checked_add(value(costs, c.row))
            {
                let proposal = (cost, count + 1, c.row, c.start);
                best = Some(best.map_or(proposal, |old| proposal.min(old)));
            }
        }
        if let Some((cost, count, row, a)) = best {
            scores[j + 1] = Some((cost, count));
            back[j + 1] = Some((a, row));
        }
    }
    if cover::reconstruct(&back, mask) {
        Ok(())
    } else {
        Err(IntervalError::CostOverflow)
    }
}
