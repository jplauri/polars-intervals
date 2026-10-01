use super::{IntervalError, cover, validate_lengths};
use std::{cmp::Reverse, collections::BinaryHeap};

/// Select a minimum-cardinality dominating set of half-open intervals.
///
/// Each selected row dominates itself and every overlapping nonempty row.
/// Touching intervals do not overlap. Every empty interval is isolated and
/// must be selected, even when several empties have identical coordinates.
/// The returned mask follows original row order. Empty input returns an empty
/// mask. This unit-cost entry point does not allocate a vector of ones.
///
/// A direct greedy sweep selects the furthest-reaching neighbor of the
/// earliest-ending undominated row. Sorting and reconstruction take
/// `O(n log n)` time and `O(n)` additional space. Endpoints require comparisons
/// only. Ties are deterministic for identical input, but the particular tied
/// mask is not guaranteed across releases.
///
/// # Errors
/// Rejects unequal endpoint lengths and reversed intervals, reporting original
/// row indices. Every row is validated before optimization.
///
/// ```
/// let selected = intervals_core::minimum_dominating_set(&[0, 3, 6], &[4, 7, 10])?;
/// assert_eq!(selected, [false, true, false]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn minimum_dominating_set<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
) -> Result<Vec<bool>, IntervalError> {
    validate_lengths(starts, ends)?;
    let mut mask = vec![false; starts.len()];
    let mut arrivals = Vec::new();
    for i in 0..starts.len() {
        if starts[i] > ends[i] {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        if starts[i] == ends[i] {
            mask[i] = true;
        } else {
            arrivals.push(i);
        }
    }
    let mut departures = arrivals.clone();
    arrivals.sort_unstable_by_key(|&i| (starts[i], i));
    departures.sort_unstable_by_key(|&i| (ends[i], i));
    let (mut next, mut best, mut frontier) = (0, None, None);
    for demand in departures {
        if frontier.is_some_and(|end| starts[demand] < end) {
            continue;
        }
        while next < arrivals.len() && starts[arrivals[next]] < ends[demand] {
            let row = arrivals[next];
            if best.is_none_or(|old| ends[row] > ends[old] || (ends[row] == ends[old] && row < old))
            {
                best = Some(row);
            }
            next += 1;
        }
        // Exchange: let d be the earliest remaining demand, o an optimal
        // neighbor, and g our furthest-reaching neighbor. Every remaining x
        // has end(x) >= end(d) > start(g). If o dominates x, start(x) < end(o)
        // <= end(g), so replacing o by g preserves domination and count.
        // After choosing g, the remaining demands are dominated precisely
        // when their starts precede end(g): the sweep's frontier invariant.
        // Candidates remain eligible even when they were already dominated.
        let row = best.expect("a nonempty demand is its own candidate");
        debug_assert!(ends[row] > starts[demand]);
        mask[row] = true;
        frontier = Some(ends[row]);
    }
    Ok(mask)
}

/// Select an exact minimum-cost dominating set, breaking ties by fewer rows.
///
/// Uses the same self-domination, half-open adjacency, mandatory empty rows,
/// original-order mask, and deterministic tie semantics as
/// [`minimum_dominating_set`]. Costs are nonnegative integers convertible
/// losslessly to `i128`. Zero-cost rows remain candidates and demands; a
/// nonempty input always selects at least one row.
///
/// Distinct inclusion-minimal nonempty geometries are sufficient demands:
/// every original interval contains a minimal target, and any interval
/// overlapping that target overlaps the original interval too. Sorted minimal
/// targets have strictly increasing starts and ends, so every candidate
/// overlaps one consecutive target-index block. Covering those blocks with
/// a heap-based prefix dynamic program preserves feasibility, cost, and count.
/// All original nonempty rows remain candidates, including containing rows.
/// Uniform costs, including all-zero costs, use the cardinality greedy sweep.
///
/// Including reduction and reconstruction, takes `O(n log n)` time and `O(n)`
/// additional space. No coordinate arithmetic or graph edges are needed.
///
/// # Errors
/// Besides endpoint validation, rejects mismatched cost lengths and negative
/// costs at their original row index. All rows and costs are validated before
/// optimization. Checked arithmetic discards overflowing alternatives; returns
/// [`IntervalError::CostOverflow`] only when the optimum, including mandatory
/// empty rows, exceeds `i128::MAX`.
///
/// ```
/// let selected = intervals_core::minimum_cost_dominating_set(
///     &[0, 3, 6], &[4, 7, 10], &[1, 10, 1],
/// )?;
/// assert_eq!(selected, [true, false, true]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn minimum_cost_dominating_set<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    costs: &[W],
) -> Result<Vec<bool>, IntervalError>
where
    i128: From<W>,
{
    validate_lengths(starts, ends)?;
    cover::validate_costs(costs, starts.len())?;
    if costs.first().is_none_or(|&first| {
        costs
            .iter()
            .all(|&cost| i128::from(cost) == i128::from(first))
    }) {
        let mask = minimum_dominating_set(starts, ends)?;
        check_selected_cost(&mask, costs)?;
        return Ok(mask);
    }
    let reduction = reduce(starts, ends)?;
    let mut mask = vec![false; starts.len()];
    for &row in &reduction.empties {
        mask[row] = true;
    }
    prefix_heap(&reduction, Some(costs), &mut mask)?;
    check_selected_cost(&mask, costs)?;
    Ok(mask)
}

pub(crate) struct Reduction<T> {
    pub targets: Vec<(T, T)>,
    // Starts are nondecreasing: mapping reuses the extraction's sorted order.
    pub blocks: Vec<cover::Candidate<usize>>,
    pub empties: Vec<usize>,
}

pub(crate) fn reduce<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
) -> Result<Reduction<T>, IntervalError> {
    validate_lengths(starts, ends)?;
    let mut rows = Vec::new();
    let mut empties = Vec::new();
    for (row, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: row });
        }
        if start == end {
            empties.push(row);
        } else {
            rows.push(cover::Candidate { start, end, row });
        }
    }
    rows.sort_unstable_by(|a, b| {
        b.start
            .cmp(&a.start)
            .then(a.end.cmp(&b.end))
            .then(a.row.cmp(&b.row))
    });
    let mut targets = Vec::new();
    let mut minimum_end = None;
    for row in &rows {
        // Equal geometry is deduplicated only among demands. Equal starts
        // retain their smallest end; equal ends retain their greatest start.
        if minimum_end.is_none_or(|end| row.end < end) {
            targets.push((row.start, row.end));
            minimum_end = Some(row.end);
        }
    }
    targets.reverse();
    let blocks = rows
        .into_iter()
        .rev()
        .map(|row| {
            let start = targets.partition_point(|&(_, end)| end <= row.start);
            let end = targets.partition_point(|&(start, _)| start < row.end);
            // Every nonempty row contains a minimal target.
            debug_assert!(start < end);
            cover::Candidate {
                start,
                end,
                row: row.row,
            }
        })
        .collect();
    Ok(Reduction {
        targets,
        blocks,
        empties,
    })
}

/// F[j] covers targets before j and may also cover later targets. For a block
/// [a,b) containing j, F[a] plus that row covers through j, giving an upper
/// bound. Conversely remove a row covering j from an optimal prefix cover:
/// every target before its a remains covered, so the remainder costs/counts
/// at least F[a]. These two bounds prove the recurrence. Nonnegative costs
/// and the count tie-break exclude a row starting at or after the prefix
/// length; therefore F[a] does not already select this row.
///
/// Insert each proposal only at a, when F[a] is known. Expiration b controls
/// feasibility, not ordering. Buried expired entries are deliberately lazy:
/// heap storage can reach O(n), even when the current active set is small.
pub(crate) fn prefix_heap<T, W: Copy>(
    reduction: &Reduction<T>,
    costs: Option<&[W]>,
    mask: &mut [bool],
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let mut heap = BinaryHeap::new();
    let mut back = vec![None; reduction.targets.len() + 1];
    let mut previous = Some((0i128, 0usize));
    let mut next = 0;
    for j in 0..reduction.targets.len() {
        while next < reduction.blocks.len() && reduction.blocks[next].start == j {
            let c = reduction.blocks[next];
            let row_cost = costs.map_or(1, |costs| i128::from(costs[c.row]));
            if let Some((cost, count)) = previous
                && let Some(cost) = cost.checked_add(row_cost)
            {
                heap.push(Reverse((cost, count + 1, c.row, c.start, c.end)));
            }
            // An overflowing prefix cannot return to range under nonnegative
            // costs. Skip this alternative; another may still be representable.
            next += 1;
        }
        while heap.peek().is_some_and(|Reverse(proposal)| proposal.4 <= j) {
            heap.pop();
        }
        previous = heap.peek().map(|&Reverse((cost, count, row, a, _))| {
            debug_assert!(a <= j);
            back[j + 1] = Some((a, row));
            (cost, count)
        });
    }
    if cover::reconstruct(&back, mask) {
        Ok(())
    } else {
        // Every valid domination instance is feasible by selecting all rows.
        Err(IntervalError::CostOverflow)
    }
}

pub(crate) fn check_selected_cost<W: Copy>(mask: &[bool], costs: &[W]) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let mut total = 0i128;
    for (&selected, &cost) in mask.iter().zip(costs) {
        if selected {
            total = total
                .checked_add(i128::from(cost))
                .ok_or(IntervalError::CostOverflow)?;
        }
    }
    Ok(())
}
