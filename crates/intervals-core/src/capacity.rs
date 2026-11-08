use crate::capacity_flow::{Network, add, solve_components};
use crate::{IntervalError, check_len, max_weight_non_overlapping, validate_lengths};

/// Select a globally maximum-weight subset subject to maximum simultaneous capacity.
///
/// Returns a Boolean mask in original row order for half-open `[start, end)`
/// intervals. Positive empty intervals are always selected and consume no
/// capacity. Nonpositive rows are omitted. Ties are deterministic for identical
/// input, but the particular optimal mask is not a stable contract.
///
/// Capacity zero selects only positive empty intervals. Capacity one delegates
/// to [`max_weight_non_overlapping`]. Other capacities use exact min-cost flow
/// on one capacity segment, after filtering irrelevant rows, detecting sufficient
/// capacity, and separating independent overlap components. Substantial component
/// workloads use at most eight scoped workers; small instances stay serial.
/// No conflict graph is constructed. Endpoints require ordering and `Sync`,
/// without arithmetic. Results are independent of worker completion order.
///
/// Takes `O(n log n + sum(k_c * n_c * log(n_c + 1)))` time and `O(n)` space,
/// where `n_c` is the number of positive non-empty rows in each constrained
/// component and `k_c = capacity < concurrency_c`. Unconstrained components
/// require no flow. Capacity zero takes `O(n)` time; capacity one `O(n log n)`.
///
/// # Errors
///
/// Rejects unequal endpoint/weight lengths and the first reversed original row,
/// including rows with nonpositive weights or capacity zero. Weights convert
/// losslessly to `i128`; arithmetic and the selected objective are checked and
/// return [`IntervalError::WeightOverflow`] on overflow.
///
/// # Examples
///
/// ```
/// use intervals_core::max_weight_with_capacity;
/// let mask = max_weight_with_capacity(
///     &[0, 0, 4, 7], &[10, 4, 7, 10], &[15, 10, 10, 10], 2,
/// )?;
/// assert_eq!(mask, [true; 4]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_weight_with_capacity<T, W>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
    capacity: usize,
) -> Result<Vec<bool>, IntervalError>
where
    T: Ord + Copy + Sync,
    W: Copy,
    i128: From<W>,
{
    if starts.is_empty() && ends.is_empty() && weights.is_empty() {
        return Ok(Vec::new());
    }
    if capacity == 1 {
        return max_weight_non_overlapping(starts, ends, weights);
    }
    let Prepared { rows, mut mask } = prepare(starts, ends, weights, capacity)?;
    if concurrency(&rows) <= capacity {
        for row in &rows {
            mask[row.index] = true;
        }
    } else {
        let ranges = components(&rows);
        // Release measurements justify threads only beyond this amount of flow
        // work. Bound workers, and keep small/one-component instances serial.
        let workers = if ranges.len() >= 8 && rows.len().saturating_mul(capacity) >= 64_000 {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
        } else {
            1
        };
        solve_components(&rows, &ranges, &mut mask, workers, |component, emit| {
            solve_component(component, capacity, emit)
        })?;
    }
    check_objective(weights, &mask)?;
    Ok(mask)
}

fn solve_component<T: Ord + Copy>(
    rows: &[Row<T>],
    capacity: usize,
    emit: &mut dyn FnMut(usize),
) -> Result<(), IntervalError> {
    if concurrency(rows) <= capacity {
        rows.iter().for_each(|row| emit(row.index));
        Ok(())
    } else {
        Network::one_segment(rows, capacity).select(rows, emit)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Row<T> {
    pub start: T,
    pub end: T,
    pub weight: i128,
    pub index: usize,
}

pub(crate) struct Prepared<T> {
    pub rows: Vec<Row<T>>,
    pub mask: Vec<bool>,
}

pub(crate) fn prepare<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
    capacity: usize,
) -> Result<Prepared<T>, IntervalError>
where
    i128: From<W>,
{
    validate_lengths(starts, ends)?;
    check_len(("intervals", starts.len()), ("weights", weights.len()))?;
    let mut mask = vec![false; starts.len()];
    let mut rows = Vec::new();
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        let weight = i128::from(weights[index]);
        if weight > 0 {
            if start == end {
                mask[index] = true;
            } else if capacity > 0 {
                rows.push(Row {
                    start,
                    end,
                    weight,
                    index,
                });
            }
        }
    }
    rows.sort_unstable_by_key(|r| (r.start, r.end, r.index));
    Ok(Prepared { rows, mask })
}

pub(crate) fn check_objective<W: Copy>(weights: &[W], mask: &[bool]) -> Result<i128, IntervalError>
where
    i128: From<W>,
{
    weights
        .iter()
        .zip(mask)
        .filter(|(_, chosen)| **chosen)
        .try_fold(0i128, |sum, (&w, _)| add(sum, i128::from(w)))
}

// Rows are start ordered. Touching intervals belong to different components.
pub(crate) fn components<T: Ord + Copy>(rows: &[Row<T>]) -> Vec<std::ops::Range<usize>> {
    let mut result = Vec::new();
    if let Some(first) = rows.first() {
        let (mut begin, mut end) = (0, first.end);
        for (i, row) in rows.iter().enumerate().skip(1) {
            if row.start >= end {
                result.push(begin..i);
                begin = i;
            }
            end = end.max(row.end);
        }
        result.push(begin..rows.len());
    }
    result
}

fn concurrency<T: Ord + Copy>(rows: &[Row<T>]) -> usize {
    let mut ends: Vec<_> = rows.iter().map(|r| r.end).collect();
    ends.sort_unstable();
    let (mut ended, mut peak) = (0, 0);
    for (i, row) in rows.iter().enumerate() {
        while ended < ends.len() && ends[ended] <= row.start {
            ended += 1;
        }
        peak = peak.max(i + 1 - ended);
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn solve_parallel(
        rows: &[Row<i32>],
        ranges: &[std::ops::Range<usize>],
        capacity: usize,
        mask: &mut [bool],
        workers: usize,
    ) -> Result<(), IntervalError> {
        solve_components(rows, ranges, mask, workers, |component, emit| {
            solve_component(component, capacity, emit)
        })
    }

    fn invariants(s: &[i32], e: &[i32], w: &[i64], k: usize) {
        let prepared = prepare(s, e, w, k).unwrap();
        if prepared.rows.is_empty() {
            return;
        }
        // Timelines clamp capacity to the row count, for profiles and one segment.
        let k = k.min(prepared.rows.len());
        let mut net = Network::one_segment(&prepared.rows, k);
        let initial: Vec<_> = net.edges.iter().map(|e| e.capacity).collect();
        net.solve().unwrap();
        let n = net.offsets.len() - 1;
        let mut balance = vec![0i128; n];
        let mut cost = 0;
        for edge in (0..net.edges.len()).step_by(2) {
            let forward = &net.edges[edge];
            let reverse = &net.edges[edge ^ 1];
            assert_eq!(forward.cost, -reverse.cost);
            assert_eq!(forward.capacity + reverse.capacity, initial[edge]);
            assert!(forward.capacity <= initial[edge]);
            let flow = reverse.capacity as i128;
            balance[reverse.to] -= flow;
            balance[forward.to] += flow;
            cost += flow * forward.cost;
        }
        assert_eq!(balance[0], -(k as i128));
        assert_eq!(balance[n - 1], k as i128);
        assert!(balance[1..n - 1].iter().all(|&v| v == 0));
        for v in 0..n {
            for &edge in &net.adjacent[net.offsets[v]..net.offsets[v + 1]] {
                assert_eq!(net.edges[edge ^ 1].to, v);
            }
        }
        let mut mask = vec![false; s.len()];
        net.reconstruct(&prepared.rows, &mut mask);
        assert_eq!(cost, -check_objective(w, &mask).unwrap());
        for (row, [_, reverse]) in prepared
            .rows
            .iter()
            .zip(net.edges[net.interval_start..].as_chunks::<2>().0)
        {
            assert_eq!(mask[row.index], reverse.capacity == 1);
        }
        let mut again = Network::one_segment(&prepared.rows, k);
        again.solve().unwrap();
        assert_eq!(
            net.edges.iter().map(|e| e.capacity).collect::<Vec<_>>(),
            again.edges.iter().map(|e| e.capacity).collect::<Vec<_>>()
        );
    }

    #[test]
    fn residual_capacity_conservation_cost_and_ties() {
        for k in [1, 2, 4, 16, 64] {
            invariants(&[0, 0, 1, 2, 3], &[5, 2, 3, 4, 5], &[4, 3, 2, 2, 3], k);
            invariants(&[0; 8], &[1; 8], &[2; 8], k);
        }
    }

    #[test]
    fn parallel_matches_serial_and_propagates_overflow() {
        let s: Vec<_> = (0..128).map(|i| i / 8 * 20 + i % 8).collect();
        let e: Vec<_> = s.iter().map(|s| s + 5).collect();
        let w: Vec<_> = (0..128).map(|i| i as i128 % 7 + 1).collect();
        let prepared = prepare(&s, &e, &w, 2).unwrap();
        let ranges = components(&prepared.rows);
        let mut serial = prepared.mask.clone();
        for range in &ranges {
            solve_component(&prepared.rows[range.clone()], 2, &mut |i| serial[i] = true).unwrap();
        }
        for workers in [1, 2, 8] {
            let mut parallel = prepared.mask.clone();
            solve_parallel(&prepared.rows, &ranges, 2, &mut parallel, workers).unwrap();
            assert_eq!(parallel, serial);
        }
        let prepared = prepare(&[0, 0, 0, 1], &[2, 2, 1, 2], &[1, 1, i128::MAX, 2], 2).unwrap();
        let mut mask = prepared.mask;
        assert_eq!(
            solve_parallel(&prepared.rows, &components(&prepared.rows), 2, &mut mask, 2),
            Err(IntervalError::WeightOverflow)
        );
    }

    proptest! {
        #[test]
        fn residual_invariants(rows in prop::collection::vec((-4i32..=4, 1i32..=6, 1i64..=20), 1..=20), k in 1usize..=8) {
            let s: Vec<_> = rows.iter().map(|r| r.0).collect();
            let e: Vec<_> = rows.iter().map(|r| r.0 + r.1).collect();
            let w: Vec<_> = rows.iter().map(|r| r.2).collect();
            invariants(&s, &e, &w, k);
        }
    }
}
