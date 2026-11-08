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
    prefix_heap(&reduction, costs, &mut mask)?;
    check_selected_cost(&mask, costs)?;
    Ok(mask)
}

struct Reduction<T> {
    targets: Vec<(T, T)>,
    // Starts are nondecreasing: mapping reuses the extraction's sorted order.
    blocks: Vec<cover::Candidate<usize>>,
    empties: Vec<usize>,
}

fn reduce<T: Ord + Copy>(starts: &[T], ends: &[T]) -> Result<Reduction<T>, IntervalError> {
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
fn prefix_heap<T, W: Copy>(
    reduction: &Reduction<T>,
    costs: &[W],
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
            if let Some((cost, count)) = previous
                && let Some(cost) = cost.checked_add(i128::from(costs[c.row]))
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

fn check_selected_cost<W: Copy>(mask: &[bool], costs: &[W]) -> Result<(), IntervalError>
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

// The public optimizers share this reduction, so it needs its own oracle.
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Self-domination never goes through overlap, so even duplicate empty
    // rows remain mandatory in the original graph.
    fn dominates(s: &[i64], e: &[i64], selected: &[bool]) -> bool {
        (0..s.len()).all(|i| {
            selected[i]
                || (0..s.len()).any(|j| {
                    selected[j] && s[i] < e[i] && s[j] < e[j] && s[i] < e[j] && s[j] < e[i]
                })
        })
    }

    // Pairwise strict containment is independent of optimized target extraction.
    // Equal geometries are deduplicated ONLY after testing strict containment.
    fn naive_targets(s: &[i64], e: &[i64]) -> Vec<(i64, i64)> {
        let mut targets: Vec<_> = (0..s.len())
            .filter(|&i| {
                s[i] < e[i]
                    && !(0..s.len()).any(|j| {
                        s[j] < e[j]
                            && s[i] <= s[j]
                            && e[j] <= e[i]
                            && (s[i] != s[j] || e[i] != e[j])
                    })
            })
            .map(|i| (s[i], e[i]))
            .collect();
        targets.sort_unstable();
        targets.dedup();
        targets
    }

    fn check_reduction(s: &[i64], e: &[i64]) {
        let reduction = reduce(s, e).unwrap();
        assert_eq!(reduction.targets, naive_targets(s, e));
        assert!(
            reduction
                .targets
                .windows(2)
                .all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1)
        );
        assert_eq!(
            reduction.empties,
            (0..s.len()).filter(|&i| s[i] == e[i]).collect::<Vec<_>>()
        );
        let mut rows: Vec<_> = reduction.blocks.iter().map(|b| b.row).collect();
        rows.sort_unstable();
        assert_eq!(
            rows,
            (0..s.len()).filter(|&i| s[i] < e[i]).collect::<Vec<_>>()
        );
        for block in &reduction.blocks {
            assert!(block.start < block.end);
            for (j, &(ts, te)) in reduction.targets.iter().enumerate() {
                assert_eq!(
                    block.start <= j && j < block.end,
                    s[block.row] < te && ts < e[block.row],
                    "row {} target {j}",
                    block.row
                );
            }
        }
        for bits in 0usize..1 << s.len() {
            let mask: Vec<_> = (0..s.len()).map(|i| bits & (1 << i) != 0).collect();
            let reduced = reduction.empties.iter().all(|&i| mask[i])
                && (0..reduction.targets.len()).all(|j| {
                    reduction
                        .blocks
                        .iter()
                        .any(|b| mask[b.row] && b.start <= j && j < b.end)
                });
            assert_eq!(dominates(s, e, &mask), reduced, "subset {bits:b}");
        }
    }

    #[test]
    fn target_boundaries_and_identical_blocks() {
        // [1,4) touches the second target; [3,8) touches the third. Rows 6/7
        // have the same block despite different geometry.
        check_reduction(&[0, 4, 8, 12, 1, 0, 3, 3], &[2, 6, 10, 14, 4, 7, 7, 8]);
    }

    fn interval() -> impl Strategy<Value = (i64, i64)> {
        prop_oneof![
            5 => (-6i64..=6, -6i64..=6).prop_map(|(a, b)| (a.min(b), a.max(b))),
            2 => (-5i64..=5, 1i64..=2).prop_map(|(block, len)| (4 * block, 4 * block + len)),
            2 => (0i64..=6).prop_map(|radius| (-radius, radius + 1)),
            1 => (-6i64..=6).prop_map(|x| (x, x)),
        ]
    }

    proptest! {
        #[test]
        fn reduction_preserves_every_subset_and_matches_naive_targets_and_adjacency(
            rows in prop_oneof![
                4 => prop::collection::vec(interval(), 0..=7),
                1 => (interval(), 0usize..=7).prop_map(|(row, n)| vec![row; n]),
            ],
        ) {
            let (s, e): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
            check_reduction(&s, &e);
        }
    }
}
