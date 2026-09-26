use crate::IntervalError;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Candidate<T> {
    pub start: T,
    pub end: T,
    pub row: usize,
}

pub(crate) fn prepare<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    left: T,
    right: T,
) -> Result<Vec<Candidate<T>>, IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    if left > right {
        return Err(IntervalError::InvalidTarget);
    }
    let mut candidates = Vec::new();
    for (row, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: row });
        }
        let (start, end) = (start.max(left), end.min(right));
        if start < end {
            candidates.push(Candidate { start, end, row });
        }
    }
    Ok(candidates)
}

/// Select the fewest intervals whose union continuously covers `[target_start, target_end)`.
///
/// Returns a Boolean mask in original row order. Empty input intervals are never
/// selected; an empty target returns all false after validating all input rows.
/// Touching intervals chain perfectly. Intervals may extend outside the target.
/// The furthest-reaching greedy sweep is globally optimal, takes `O(n log n)`
/// time including sorting and `O(n)` space, and uses comparisons only.
/// Ties prefer the lowest original row index after clipping to the target.
///
/// # Errors
/// Rejects unequal lengths, reversed intervals/targets, and infeasible coverage
/// ([`IntervalError::InfeasibleCover`]).
///
/// ```
/// let mask = intervals_core::minimum_cover(&[0, 0, 4, 6, 7], &[4, 6, 7, 10, 10], 0, 10)?;
/// assert_eq!(mask, [false, true, false, true, false]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn minimum_cover<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    target_start: T,
    target_end: T,
) -> Result<Vec<bool>, IntervalError> {
    let mut candidates = prepare(starts, ends, target_start, target_end)?;
    candidates.sort_unstable_by_key(|c| (c.start, c.row));
    let mut mask = vec![false; starts.len()];
    greedy(&candidates, target_start, target_end, &mut mask)?;
    Ok(mask)
}

pub(crate) fn greedy<T: Ord + Copy>(
    candidates: &[Candidate<T>],
    mut frontier: T,
    right: T,
    mask: &mut [bool],
) -> Result<(), IntervalError> {
    let mut next = 0;
    while frontier < right {
        let mut best: Option<Candidate<T>> = None;
        while next < candidates.len() && candidates[next].start <= frontier {
            let c = candidates[next];
            if c.end > frontier
                && best.is_none_or(|b| c.end > b.end || (c.end == b.end && c.row < b.row))
            {
                best = Some(c);
            }
            next += 1;
        }
        let best = best.ok_or(IntervalError::InfeasibleCover)?;
        mask[best.row] = true;
        frontier = best.end;
        // All scanned candidates end at or before the new frontier, so none
        // can be useful again. No active set or heap is needed.
    }
    Ok(())
}

// Tuple order defines the objective, then deterministic predecessor ties.
pub(crate) type Best = Option<(i128, usize, usize)>;

pub(crate) fn better(a: Best, b: Best) -> Best {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        _ => a.or(b),
    }
}

pub(crate) struct Fenwick {
    tree: Vec<Best>,
}

impl Fenwick {
    pub fn new(len: usize) -> Self {
        Self {
            tree: vec![None; len + 1],
        }
    }

    pub fn update(&mut self, coordinate: usize, value: Best) {
        let mut i = self.tree.len() - 1 - coordinate;
        while i < self.tree.len() {
            let best = better(self.tree[i], value);
            if best == self.tree[i] {
                break; // Every ancestor already contains this node's minimum.
            }
            self.tree[i] = best;
            i += i.isolate_lowest_one();
        }
    }

    pub fn suffix(&self, coordinate: usize) -> Best {
        let mut i = self.tree.len() - 1 - coordinate;
        let mut best = None;
        while i > 0 {
            best = better(best, self.tree[i]);
            i &= i - 1;
        }
        best
    }
}

pub(crate) fn validate_costs<W: Copy>(costs: &[W], len: usize) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    if costs.len() != len {
        return Err(IntervalError::CostLengthMismatch {
            intervals_len: len,
            costs_len: costs.len(),
        });
    }
    for (index, &cost) in costs.iter().enumerate() {
        if i128::from(cost) < 0 {
            return Err(IntervalError::NegativeCost { index });
        }
    }
    Ok(())
}

pub(crate) fn coordinates<T: Ord + Copy>(candidates: &[Candidate<T>], left: T) -> Vec<T> {
    let mut coordinates = Vec::with_capacity(candidates.len() + 1);
    coordinates.push(left);
    coordinates.extend(candidates.iter().map(|c| c.end));
    coordinates.dedup(); // Candidates are sorted by end; all ends exceed left.
    coordinates
}

pub(crate) struct Solution {
    pub back: Vec<Option<(usize, usize)>>,
    pub overflowed: bool,
}

pub(crate) fn dynamic_program<T: Ord + Copy, W: Copy>(
    candidates: &[Candidate<T>],
    coordinates: &[T],
    costs: &[W],
) -> Solution
where
    i128: From<W>,
{
    let mut tree = Fenwick::new(coordinates.len());
    tree.update(0, Some((0, 0, 0)));
    let mut back = vec![None; coordinates.len()];
    let mut overflowed = false;
    let mut next = 0;
    for r in 1..coordinates.len() {
        let mut best: Option<(i128, usize, usize, usize)> = None;
        while next < candidates.len() && candidates[next].end == coordinates[r] {
            let c = candidates[next];
            let l = coordinates.partition_point(|&x| x < c.start);
            if let Some((cost, count, predecessor)) = tree.suffix(l) {
                if let Some(cost) = cost.checked_add(i128::from(costs[c.row])) {
                    let proposal = (cost, count + 1, c.row, predecessor);
                    best = Some(best.map_or(proposal, |b| b.min(proposal)));
                } else {
                    // Nonnegative costs can never return to the i128 range.
                    // Ignore this path, not another representable optimum.
                    overflowed = true;
                }
            }
            next += 1;
        }
        if let Some((cost, count, row, predecessor)) = best {
            debug_assert!(predecessor < r);
            back[r] = Some((predecessor, row));
            // Publish only after the entire equal-right-end batch is queried.
            tree.update(r, Some((cost, count, r)));
        }
    }
    Solution { back, overflowed }
}

pub(crate) fn reconstruct(back: &[Option<(usize, usize)>], mask: &mut [bool]) -> bool {
    let mut frontier = back.len() - 1;
    while frontier != 0 {
        let Some((predecessor, row)) = back[frontier] else {
            return false;
        };
        debug_assert!(predecessor < frontier);
        mask[row] = true;
        frontier = predecessor;
    }
    true
}

/// Select a minimum-cost continuous cover, breaking equal-cost ties by fewer intervals.
///
/// Same target, interval, validation and mask semantics as [`minimum_cover`].
/// Costs must be nonnegative integers convertible losslessly to `i128`.
/// Accumulation is checked: overflowing paths are discarded; an error is
/// returned only if a feasible target has no representable optimal objective.
/// All costs are validated, including rows outside the target or an empty target.
///
/// Uses exact frontier dynamic programming, a reversed Fenwick suffix-min tree,
/// and backpointer reconstruction. Equal-right-end batches cannot chain into
/// themselves. Takes `O(n log n)` time and `O(n)` space. Ties use original row
/// and predecessor coordinate order; a particular tied mask is not an API guarantee.
///
/// # Errors
/// Besides [`minimum_cover`]'s errors: [`IntervalError::CostLengthMismatch`],
/// [`IntervalError::NegativeCost`], and [`IntervalError::CostOverflow`].
///
/// ```
/// let mask = intervals_core::minimum_cost_cover(&[0, 0, 5], &[10, 5, 10], &[100, 10, 10], 0, 10)?;
/// assert_eq!(mask, [false, true, true]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn minimum_cost_cover<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    costs: &[W],
    target_start: T,
    target_end: T,
) -> Result<Vec<bool>, IntervalError>
where
    i128: From<W>,
{
    validate_costs(costs, starts.len())?;
    let mut candidates = prepare(starts, ends, target_start, target_end)?;
    if target_start == target_end {
        return Ok(vec![false; starts.len()]);
    }
    candidates.sort_unstable_by_key(|c| (c.end, c.row));
    let coordinates = coordinates(&candidates, target_start);
    if coordinates.last() != Some(&target_end) {
        return Err(IntervalError::InfeasibleCover);
    }
    let solution = dynamic_program(&candidates, &coordinates, costs);
    let mut mask = vec![false; starts.len()];
    if !reconstruct(&solution.back, &mut mask) {
        if solution.overflowed && minimum_cover(starts, ends, target_start, target_end).is_ok() {
            return Err(IntervalError::CostOverflow);
        }
        return Err(IntervalError::InfeasibleCover);
    }
    Ok(mask)
}
