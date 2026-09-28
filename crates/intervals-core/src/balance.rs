//! Cardinality balancing on a fixed minimum palette.
use crate::{IntervalError, assign_lanes};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Why bounded balancing stopped. Only `PairwiseFixedPoint` certifies a complete
/// no-improvement pass; it does not certify a globally optimal coloring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BalanceStopReason {
    /// The elementary cardinality lower bound was reached.
    Equity,
    /// Every unequal pair was checked without an improving recoloring.
    PairwiseFixedPoint,
    /// The next atomic operation did not fit the remaining work budget.
    BudgetExhausted,
    /// Checked scratch allocation could not be satisfied.
    ScratchLimit,
}

/// Lightweight counters for tests and benchmarks, outside the expression API.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalanceDiagnostics {
    pub work: u64,
    pub pairs: u64,
    pub flips: u64,
    pub skips: u64,
    pub stop_reason: BalanceStopReason,
}

/// A coloring and its bounded-refinement diagnostics.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalanceResult {
    pub lanes: Vec<u32>,
    pub diagnostics: BalanceDiagnostics,
}

/// Construct or improve a proper minimum-lane coloring, balancing row counts.
///
/// Minimizes `(max(counts)-min(counts), sum(counts²))` lexicographically among
/// the visited candidates, using exact integers. With `initial_lanes=None`,
/// constructs a coloring whose score never worsens [`assign_lanes`]. With
/// `Some(lanes)`, validates and improves that supplied coloring without changing
/// its palette or worsening its score. Supplied IDs must be proper, contiguous
/// from zero and use the true minimum number of lanes.
/// No global optimum or approximation ratio is promised.
/// Empty rows count toward balance and may use any existing lane, unlike
/// [`assign_lanes`], which puts them in lane zero. Empty input uses zero lanes;
/// nonempty input uses `max(1, maximum_nonempty_concurrency)` lanes.
///
/// Zero work returns the old baseline for `None`, or the supplied coloring
/// unchanged for `Some`, after validating all inputs. Validation also runs on
/// already equitable inputs. Positive work with `None` constructs forward and
/// backward least-loaded-free-lane seeds, stopping early at equity, and repairs
/// the best seed with exact two-lane component repartitioning. `Some` repairs
/// only the supplied coloring. The fixed construction and
/// preprocessing cost is `O(n log n)` time and `O(n + k)` space. Refinement uses
/// at most `max_work` deterministic units and `O(n + k)` scratch space: units
/// charge pair enumeration, row visits, bitset words, reconstruction and lane
/// scans/sorting (with conservative preflight charges for atomic operations).
/// This is a search-work limit, not a wall-clock or total-construction limit.
/// A pair operation that cannot fit leaves the incumbent intact.
///
/// Identical inputs/options give identical output in original row order. Only
/// `Ord + Copy` endpoints are needed; backward construction never negates them.
/// A completed exact pair repair is globally optimal when there are two lanes;
/// for three or more lanes even a pairwise fixed point can be globally suboptimal.
///
/// # Errors
///
/// Endpoint validation and errors are identical to [`assign_lanes`]. A supplied
/// coloring additionally rejects wrong label lengths, sparse/out-of-range IDs,
/// overlapping intervals sharing a lane, and proper colorings using more than
/// the true minimum number of lanes.
///
/// # Examples
/// ```
/// let lanes = intervals_core::assign_balanced_lanes(&[0, 1, 2, 3], &[5, 2, 3, 4], None, 100_000)?;
/// assert_eq!(lanes.iter().copied().max(), Some(1));
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
///
/// ```
/// let starts = [0, 1, 2, 3, 4];
/// let ends = [5, 2, 3, 4, 4];
/// let initial = intervals_core::assign_lanes(&starts, &ends)?;
/// let improved = intervals_core::assign_balanced_lanes(&starts, &ends, Some(&initial), 100_000)?;
/// assert_eq!(improved.len(), initial.len());
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn assign_balanced_lanes<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    initial_lanes: Option<&[u32]>,
    max_work: u64,
) -> Result<Vec<u32>, IntervalError> {
    Ok(assign_balanced_lanes_with_diagnostics(starts, ends, initial_lanes, max_work)?.lanes)
}

#[doc(hidden)]
pub fn assign_balanced_lanes_with_diagnostics<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    initial_lanes: Option<&[u32]>,
    max_work: u64,
) -> Result<BalanceResult, IntervalError> {
    // Determine the true minimum palette and validate every endpoint before
    // either construction or supplied-coloring early returns.
    let baseline = assign_lanes(starts, ends)?;
    if let Some(lanes) = initial_lanes {
        let order = sorted_nonempty(starts, ends);
        validate_coloring(starts, ends, &order, lanes, lane_count(&baseline))?;
        return Ok(refine(starts, ends, &order, lanes.to_vec(), max_work));
    }
    let sizes = counts(&baseline);
    if max_work == 0 || equitable(&sizes, baseline.len()) {
        return Ok(refine(starts, ends, &[], baseline, 0));
    }
    let order = sorted_nonempty(starts, ends);
    let lanes = best_seed(starts, ends, &order, baseline);
    Ok(refine(starts, ends, &order, lanes, max_work))
}

fn sorted_nonempty<T: Ord + Copy>(starts: &[T], ends: &[T]) -> Vec<usize> {
    let mut order: Vec<_> = (0..starts.len()).filter(|&i| starts[i] < ends[i]).collect();
    order.sort_unstable_by_key(|&i| (starts[i], i));
    order
}

fn lane_count(lanes: &[u32]) -> usize {
    lanes.iter().copied().max().map_or(0, |v| v as usize + 1)
}

fn counts(lanes: &[u32]) -> Vec<usize> {
    let mut sizes = vec![0; lane_count(lanes)];
    for &lane in lanes {
        sizes[lane as usize] += 1;
    }
    sizes
}

fn score(sizes: &[usize]) -> (usize, u128) {
    let spread = sizes.iter().max().unwrap_or(&0) - sizes.iter().min().unwrap_or(&0);
    (
        spread,
        sizes.iter().map(|&v| (v as u128) * (v as u128)).sum(),
    )
}

fn equitable(sizes: &[usize], n: usize) -> bool {
    score(sizes).0 <= usize::from(!sizes.is_empty() && !n.is_multiple_of(sizes.len()))
}

fn validate_coloring<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    order: &[usize],
    lanes: &[u32],
    minimum: usize,
) -> Result<(), IntervalError> {
    if starts.len() != lanes.len() {
        return Err(IntervalError::LaneLengthMismatch {
            intervals_len: starts.len(),
            lanes_len: lanes.len(),
        });
    }
    // Check before any maximum-label-indexed allocation: scratch is at most n.
    for (index, &lane) in lanes.iter().enumerate() {
        if u64::from(lane) >= starts.len() as u64 {
            return Err(IntervalError::InvalidLaneId { index, lane });
        }
    }
    let sizes = counts(lanes);
    if let Some(missing) = sizes.iter().position(|&n| n == 0) {
        let index = lanes.iter().position(|&v| v as usize >= missing).unwrap();
        return Err(IntervalError::InvalidLaneId {
            index,
            lane: lanes[index],
        });
    }
    let mut previous = vec![None; sizes.len()];
    for &i in order {
        let lane = lanes[i] as usize;
        if let Some(j) = previous[lane]
            && ends[j] > starts[i]
        {
            return Err(IntervalError::LaneConflict {
                first: j,
                second: i,
            });
        }
        previous[lane] = Some(i);
    }
    if sizes.len() != minimum {
        return Err(IntervalError::NonMinimumLanes {
            actual: sizes.len(),
            minimum,
        });
    }
    Ok(())
}

fn sweep<T: Ord + Copy>(starts: &[T], ends: &[T], order: &[usize], k: usize) -> Vec<u32> {
    let mut lanes = vec![0; starts.len()];
    let mut sizes = vec![0; k];
    let mut free: BinaryHeap<_> = (0..k).map(|c| Reverse((0usize, c))).collect();
    let mut active = BinaryHeap::<Reverse<(T, usize)>>::new();
    for &i in order {
        while let Some(&Reverse((end, lane))) = active.peek()
            && end <= starts[i]
        {
            active.pop();
            free.push(Reverse((sizes[lane], lane)));
        }
        // All k colors exist from the start. At most k-1 other intervals overlap
        // this start, so at least one color is free; choosing by load keeps k minimal.
        let Reverse((_, lane)) = free.pop().expect("minimum palette has a free lane");
        lanes[i] = lane as u32;
        sizes[lane] += 1;
        active.push(Reverse((ends[i], lane)));
    }
    let mut least: BinaryHeap<_> = sizes
        .into_iter()
        .enumerate()
        .map(|(c, n)| Reverse((n, c)))
        .collect();
    for i in 0..starts.len() {
        if starts[i] == ends[i] {
            let mut next = least.peek_mut().expect("nonempty input has a lane");
            lanes[i] = next.0.1 as u32;
            next.0.0 += 1;
        }
    }
    lanes
}

fn backward_seed<T: Ord + Copy>(starts: &[T], ends: &[T], k: usize) -> Vec<u32> {
    // Reverse order, not endpoint arithmetic: unsigned and extreme endpoints work.
    let reverse_starts: Vec<_> = ends.iter().copied().map(Reverse).collect();
    let reverse_ends: Vec<_> = starts.iter().copied().map(Reverse).collect();
    let order = sorted_nonempty(&reverse_starts, &reverse_ends);
    sweep(&reverse_starts, &reverse_ends, &order, k)
}

fn best_seed<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    order: &[usize],
    mut best: Vec<u32>,
) -> Vec<u32> {
    let k = lane_count(&best);
    let mut best_score = score(&counts(&best));
    for forward in [true, false] {
        if equitable(&counts(&best), best.len()) {
            break;
        }
        let candidate = if forward {
            sweep(starts, ends, order, k)
        } else {
            backward_seed(starts, ends, k)
        };
        let candidate_score = score(&counts(&candidate));
        if candidate_score < best_score {
            best_score = candidate_score;
            best = candidate;
        }
    }
    best
}

struct Budget {
    limit: u64,
    diagnostics: BalanceDiagnostics,
}

impl Budget {
    fn charge(&mut self, amount: usize) -> bool {
        let Ok(amount) = u64::try_from(amount) else {
            return false;
        };
        if amount > self.limit - self.diagnostics.work {
            return false;
        }
        self.diagnostics.work += amount;
        true
    }
}

struct Coloring {
    lanes: Vec<u32>,
    sizes: Vec<usize>,
    rows: Vec<Vec<usize>>,
    nonempty: Vec<usize>,
}

fn refine<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    order: &[usize],
    lanes: Vec<u32>,
    max_work: u64,
) -> BalanceResult {
    let sizes = counts(&lanes);
    let k = sizes.len();
    let delta = usize::from(k > 0 && !lanes.len().is_multiple_of(k));
    let mut budget = Budget {
        limit: max_work,
        diagnostics: BalanceDiagnostics {
            work: 0,
            pairs: 0,
            flips: 0,
            skips: 0,
            stop_reason: BalanceStopReason::BudgetExhausted,
        },
    };
    if score(&sizes).0 <= delta {
        budget.diagnostics.stop_reason = BalanceStopReason::Equity;
        return BalanceResult {
            lanes,
            diagnostics: budget.diagnostics,
        };
    }
    if max_work == 0 {
        return BalanceResult {
            lanes,
            diagnostics: budget.diagnostics,
        };
    }
    let mut rows: Vec<Vec<usize>> = sizes.iter().map(|&n| Vec::with_capacity(n)).collect();
    for &i in order {
        rows[lanes[i] as usize].push(i);
    }
    let nonempty = rows.iter().map(Vec::len).collect();
    for i in 0..starts.len() {
        if starts[i] == ends[i] {
            rows[lanes[i] as usize].push(i);
        }
    }
    let mut coloring = Coloring {
        lanes,
        sizes,
        rows,
        nonempty,
    };
    let mut priority: Vec<_> = (0..k).collect();
    'search: loop {
        // Charge a conservative comparison-sort bound and scan/restart overhead.
        let sorting = k.checked_mul(k.ilog2() as usize + 2);
        if !sorting.is_some_and(|work| budget.charge(work)) {
            break;
        }
        priority.sort_unstable_by_key(|&c| (Reverse(coloring.sizes[c]), c));
        for i in 0..k {
            for j in (i + 1..k).rev() {
                if !budget.charge(1) {
                    break 'search;
                }
                let (a, b) = (priority[i], priority[j]);
                if coloring.sizes[a].abs_diff(coloring.sizes[b]) <= 1 {
                    continue;
                }
                budget.diagnostics.pairs += 1;
                match repair_pair(starts, ends, &mut coloring, a, b, &mut budget) {
                    Ok(false) => {}
                    Ok(true) => {
                        if score(&coloring.sizes).0 <= delta {
                            budget.diagnostics.stop_reason = BalanceStopReason::Equity;
                            break 'search;
                        }
                        // Restart so affected pairs are revisited; sorting and every
                        // candidate visit consume budget, even without successful moves.
                        continue 'search;
                    }
                    Err(reason) => {
                        budget.diagnostics.skips += 1;
                        budget.diagnostics.stop_reason = reason;
                        break 'search;
                    }
                }
            }
        }
        budget.diagnostics.stop_reason = BalanceStopReason::PairwiseFixedPoint;
        break;
    }
    BalanceResult {
        lanes: coloring.lanes,
        diagnostics: budget.diagnostics,
    }
}

#[derive(Clone, Copy)]
struct Component {
    start: usize,
    end: usize,
    a: usize,
    b: usize,
}

fn scratch<T>(len: usize) -> Result<Vec<T>, BalanceStopReason> {
    len.checked_mul(std::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(BalanceStopReason::ScratchLimit)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| BalanceStopReason::ScratchLimit)?;
    Ok(values)
}

fn repair_pair<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    coloring: &mut Coloring,
    a: usize,
    b: usize,
    budget: &mut Budget,
) -> Result<bool, BalanceStopReason> {
    let total = coloring.sizes[a] + coloring.sizes[b];
    if !budget.charge(total) {
        return Err(BalanceStopReason::BudgetExhausted);
    }
    let mut merged = scratch(total)?;
    let mut components: Vec<Component> = scratch(total)?;
    let (mut ai, mut bi) = (0, 0);
    let mut running_end = None;
    while ai < coloring.nonempty[a] || bi < coloring.nonempty[b] {
        let from_a = bi == coloring.nonempty[b]
            || (ai < coloring.nonempty[a] && {
                let (x, y) = (coloring.rows[a][ai], coloring.rows[b][bi]);
                (starts[x], x) <= (starts[y], y)
            });
        let row = if from_a {
            let row = coloring.rows[a][ai];
            ai += 1;
            row
        } else {
            let row = coloring.rows[b][bi];
            bi += 1;
            row
        };
        if running_end.is_none_or(|end| starts[row] >= end) {
            components.push(Component {
                start: merged.len(),
                end: merged.len(),
                a: 0,
                b: 0,
            });
            running_end = Some(ends[row]);
        } else {
            // A previous short leaf's end is insufficient for a nested star.
            running_end = Some(running_end.unwrap().max(ends[row]));
        }
        merged.push(row);
        let component = components.last_mut().unwrap();
        component.end += 1;
        if from_a {
            component.a += 1;
        } else {
            component.b += 1;
        }
    }
    for lane in [a, b] {
        // Every empty interval is its own singleton; it never bridges components.
        for &row in &coloring.rows[lane][coloring.nonempty[lane]..] {
            components.push(Component {
                start: merged.len(),
                end: merged.len() + 1,
                a: usize::from(lane == a),
                b: usize::from(lane == b),
            });
            merged.push(row);
        }
    }
    let mut weights = scratch(components.len())?;
    weights.extend(components.iter().map(|c| c.a.abs_diff(c.b)));
    let w: usize = weights.iter().sum();
    let base: usize = components.iter().map(|c| c.a.min(c.b)).sum();
    let limit = w / 2;
    let words = limit / 64 + 1;
    let cost = components
        .len()
        .checked_mul(words + 3)
        .and_then(|cost| total.checked_mul(5).and_then(|v| cost.checked_add(v)))
        .and_then(|cost| limit.checked_mul(3).and_then(|v| cost.checked_add(v)))
        .and_then(|cost| cost.checked_add(coloring.sizes.len() + words + 1));
    if !cost.is_some_and(|work| budget.charge(work)) {
        return Err(BalanceStopReason::BudgetExhausted);
    }
    let (z, selected) = subset_half(&weights, w)?;
    let new_a = base + z;
    let new_b = base + w - z;
    if new_a.abs_diff(new_b) >= coloring.sizes[a].abs_diff(coloring.sizes[b]) {
        return Ok(false);
    }
    let mut rows_a = scratch(new_a)?;
    let mut rows_b = scratch(new_b)?;
    let mut nonempty_a = 0;
    let mut nonempty_b = 0;
    // All budget and allocation checks precede mutation: exact repair is atomic.
    for (component, chosen) in components.iter().zip(selected) {
        let flip = component.a != component.b && (chosen != (component.a > component.b));
        if flip {
            budget.diagnostics.flips += 1;
        }
        for &row in &merged[component.start..component.end] {
            if flip {
                coloring.lanes[row] = if coloring.lanes[row] as usize == a {
                    b as u32
                } else {
                    a as u32
                };
            }
            if coloring.lanes[row] as usize == a {
                rows_a.push(row);
                nonempty_a += usize::from(starts[row] < ends[row]);
            } else {
                rows_b.push(row);
                nonempty_b += usize::from(starts[row] < ends[row]);
            }
        }
    }
    coloring.sizes[a] = new_a;
    coloring.sizes[b] = new_b;
    coloring.rows[a] = rows_a;
    coloring.rows[b] = rows_b;
    coloring.nonempty[a] = nonempty_a;
    coloring.nonempty[b] = nonempty_b;
    Ok(true)
}

// Exact subset sum through floor(W/2); complementary choices cover the other
// half. One predecessor per newly discovered sum gives O(W) reconstruction.
fn subset_half(weights: &[usize], total: usize) -> Result<(usize, Vec<bool>), BalanceStopReason> {
    let limit = total / 2;
    let mut bits = scratch(limit / 64 + 1)?;
    bits.resize(limit / 64 + 1, 0u64);
    bits[0] = 1;
    let mut predecessor = scratch(limit + 1)?;
    predecessor.resize(limit + 1, usize::MAX);
    for (component, &weight) in weights.iter().enumerate() {
        if weight == 0 || weight > limit {
            continue;
        }
        let (shift_words, shift_bits) = (weight / 64, weight % 64);
        for dest in (shift_words..bits.len()).rev() {
            // Descending destination words ensure every source still belongs to
            // the previous iteration, including sub-word and exact-word shifts.
            let source = dest - shift_words;
            let mut shifted = bits[source] << shift_bits;
            if shift_bits != 0 && source > 0 {
                shifted |= bits[source - 1] >> (64 - shift_bits);
            }
            let mut discovered = shifted & !bits[dest];
            bits[dest] |= shifted;
            while discovered != 0 {
                let sum = dest * 64 + discovered.trailing_zeros() as usize;
                if sum <= limit {
                    predecessor[sum] = component;
                }
                discovered &= discovered - 1;
            }
        }
    }
    let mut best = limit;
    while bits[best / 64] & (1u64 << (best % 64)) == 0 {
        best -= 1;
    }
    let mut selected = scratch(weights.len())?;
    selected.resize(weights.len(), false);
    let mut remaining = best;
    while remaining > 0 {
        let component = predecessor[remaining];
        selected[component] = true;
        remaining -= weights[component];
    }
    Ok((best, selected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn check_subset(weights: &[usize]) {
        assert!(weights.len() <= 16);
        let total = weights.iter().sum();
        let (sum, selected) = subset_half(weights, total).unwrap();
        let expected = (0..1usize << weights.len())
            .map(|mask| {
                weights
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, &w)| w)
                    .sum::<usize>()
            })
            .filter(|&sum| sum <= total / 2)
            .max()
            .unwrap();
        assert_eq!(sum, expected);
        assert_eq!(selected.len(), weights.len());
        assert_eq!(
            weights
                .iter()
                .zip(selected)
                .filter_map(|(&w, yes)| yes.then_some(w))
                .sum::<usize>(),
            sum
        );
    }

    #[test]
    fn subset_boundaries_and_no_component_reuse() {
        for weights in [
            vec![],
            vec![0, 0, 0],
            vec![10],
            vec![2, 2, 2],
            vec![8, 7, 6, 5],
            vec![63, 64, 65],
            vec![1, 63, 64, 65, 127, 128, 129],
            vec![1, 100],
            vec![3, 13, 13],
            vec![64, 64],
            vec![65, 65, 65],
            vec![2, 7, 0, 0],
        ] {
            check_subset(&weights);
        }
    }

    #[test]
    fn global_palette_available_before_late_clique() {
        let s = [0, 1, 2, 3, 4, 10, 10, 10];
        let e = [1, 2, 3, 4, 5, 11, 11, 11];
        let baseline = assign_lanes(&s, &e).unwrap();
        let forward = sweep(&s, &e, &sorted_nonempty(&s, &e), lane_count(&baseline));
        assert_eq!(score(&counts(&forward)).0, 1);
        assert!(score(&counts(&baseline)).0 > 1);
    }

    #[test]
    fn primary_spread_beats_smaller_square_sum() {
        // Same total and palette: spread wins even though Q is larger.
        assert!(score(&[2, 2, 6, 6, 6]) < score(&[1, 5, 5, 5, 6]));
        assert!(score(&[2, 2, 6, 6, 6]).1 > score(&[1, 5, 5, 5, 6]).1);
    }

    fn coloring(starts: &[i32], ends: &[i32], lanes: Vec<u32>) -> Coloring {
        let sizes = counts(&lanes);
        let mut rows = vec![Vec::new(); sizes.len()];
        for row in sorted_nonempty(starts, ends) {
            rows[lanes[row] as usize].push(row);
        }
        let nonempty = rows.iter().map(Vec::len).collect();
        for row in 0..starts.len() {
            if starts[row] == ends[row] {
                rows[lanes[row] as usize].push(row);
            }
        }
        Coloring {
            lanes,
            sizes,
            rows,
            nonempty,
        }
    }

    fn budget() -> Budget {
        Budget {
            limit: 1_000_000,
            diagnostics: BalanceDiagnostics {
                work: 0,
                pairs: 0,
                flips: 0,
                skips: 0,
                stop_reason: BalanceStopReason::BudgetExhausted,
            },
        }
    }

    #[test]
    fn accepts_q_improvement_with_unchanged_global_spread_and_rejects_equal_moves() {
        let mut lanes = Vec::new();
        for (lane, size) in [8, 4, 2, 1].into_iter().enumerate() {
            lanes.extend(std::iter::repeat_n(lane as u32, size));
        }
        let starts = vec![0; lanes.len()];
        let mut ends = starts.clone();
        // One row of each color forms a four-clique; the other rows are empty.
        // The supplied proper coloring therefore also uses the true minimum k.
        for row in [0, 8, 12, 14] {
            ends[row] = 1;
        }
        let mut coloring = coloring(&starts, &ends, lanes);
        let before = score(&coloring.sizes);
        assert!(repair_pair(&starts, &ends, &mut coloring, 1, 2, &mut budget()).unwrap());
        let after = score(&coloring.sizes);
        assert_eq!(before.0, after.0);
        assert!(after.1 < before.1);
        let lanes = coloring.lanes.clone();
        assert!(!repair_pair(&starts, &ends, &mut coloring, 1, 2, &mut budget()).unwrap());
        assert_eq!(coloring.lanes, lanes);
    }

    #[test]
    fn pair_components_split_a_connected_full_graph_and_use_running_max_end() {
        // Lane 2 bridges both stars in the full graph. In the induced 0/1 graph
        // they are separate; each long center connects all its short leaves.
        let s = [0, 1, 3, 5, 7, 20, 21, 23, 25, 0];
        let e = [10, 2, 4, 6, 8, 30, 22, 24, 26, 40];
        let mut coloring = coloring(&s, &e, vec![1, 0, 0, 0, 0, 1, 0, 0, 0, 2]);
        assert!(repair_pair(&s, &e, &mut coloring, 0, 1, &mut budget()).unwrap());
        assert_eq!(coloring.sizes, [4, 5, 1]);
        for i in 0..s.len() {
            for j in 0..i {
                if s[i] < e[j] && s[j] < e[i] {
                    assert_ne!(coloring.lanes[i], coloring.lanes[j]);
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn exact_subset_and_reconstruction(weights in prop::collection::vec(0usize..=150, 0..=12)) { check_subset(&weights); }
        #[test]
        fn accepted_pair_preserves_range_and_strictly_reduces_q(left in 1usize..100, right in 1usize..100) {
            let s = vec![0; left + right]; let e = s.clone();
            let lanes: Vec<_> = std::iter::repeat_n(0, left).chain(std::iter::repeat_n(1, right)).collect();
            let mut coloring = coloring(&s,&e,lanes);
            let before = score(&coloring.sizes);
            let changed = repair_pair(&s,&e,&mut coloring,0,1,&mut budget()).unwrap();
            let after = score(&coloring.sizes);
            prop_assert!(after.0 <= before.0);
            if changed { prop_assert!(after.1 < before.1); } else { prop_assert_eq!(after,before); }
        }
    }
}
