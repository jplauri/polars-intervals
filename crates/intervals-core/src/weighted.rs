use crate::IntervalError;

/// Select a globally maximum-weight subset of mutually non-overlapping intervals.
///
/// Returns a Boolean mask in original row order. Intervals are half-open
/// `[start, end)`; touching endpoints are compatible. Empty intervals conflict
/// with nothing: every positive empty interval is selected, including duplicates.
/// The empty subset has weight zero. Nonpositive rows are never selected.
///
/// Weights convert losslessly to `i128`; all accumulation is checked. The result
/// is deterministic for identical input, but no particular optimal subset is
/// promised across releases or row permutations when there are ties.
///
/// Sorts positive non-empty intervals by finish and start, finds predecessors
/// with a linear sweep, and reconstructs the exact dynamic-programming solution.
/// Takes `O(n log n)` time and `O(n)` additional space, including output.
/// Endpoints need only comparisons, with no subtraction or conversion.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] or
/// [`IntervalError::WeightLengthMismatch`] for unequal lengths,
/// [`IntervalError::InvalidInterval`] for the first reversed original row
/// (even if its weight is nonpositive), or [`IntervalError::WeightOverflow`]
/// if the optimal total cannot be represented in `i128`.
///
/// # Examples
///
/// ```
/// use intervals_core::max_weight_non_overlapping;
/// let mask = max_weight_non_overlapping(
///     &[0, 0, 4, 7], &[10, 4, 7, 10], &[15, 10, 10, 10],
/// )?;
/// assert_eq!(mask, [false, true, true, true]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_weight_non_overlapping<T, W>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
) -> Result<Vec<bool>, IntervalError>
where
    T: Ord + Copy,
    W: Copy,
    i128: From<W>,
{
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    if starts.len() != weights.len() {
        return Err(IntervalError::WeightLengthMismatch {
            intervals_len: starts.len(),
            weights_len: weights.len(),
        });
    }
    let mut order = Vec::with_capacity(starts.len());
    let mut selected = vec![false; starts.len()];
    let mut empty_weight = 0i128;
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        let weight = i128::from(weights[i]);
        if weight > 0 {
            if start == end {
                selected[i] = true;
                empty_weight = empty_weight
                    .checked_add(weight)
                    .ok_or(IntervalError::WeightOverflow)?;
            } else {
                order.push(i);
            }
        }
    }
    order.sort_unstable_by_key(|&i| (ends[i], starts[i], i));
    let sorted_ends: Vec<_> = order.iter().map(|&i| ends[i]).collect();
    let mut predecessors = vec![0; order.len()];
    {
        // Store finish-order positions, then sweep them in independent start order.
        // This second sort replaces one binary search per interval. Drop its
        // buffer before allocating the DP scores to keep peak memory bounded.
        let mut by_start: Vec<_> = (0..order.len()).collect();
        by_start.sort_unstable_by_key(|&j| (starts[order[j]], ends[order[j]], order[j]));
        let mut p = 0;
        for j in by_start {
            while p < sorted_ends.len() && sorted_ends[p] <= starts[order[j]] {
                p += 1;
            }
            // p is a prefix length; non-empty intervals ensure p <= j.
            predecessors[j] = p;
        }
    }
    let mut optimum = vec![0i128; order.len() + 1];
    for (j, &i) in order.iter().enumerate() {
        let include = optimum[predecessors[j]]
            .checked_add(i128::from(weights[i]))
            .ok_or(IntervalError::WeightOverflow)?;
        optimum[j + 1] = optimum[j].max(include);
    }
    empty_weight
        .checked_add(optimum[order.len()])
        .ok_or(IntervalError::WeightOverflow)?;
    let mut j = order.len();
    while j > 0 {
        // Skip the current row on an objective tie.
        if optimum[j] > optimum[j - 1] {
            selected[order[j - 1]] = true;
            j = predecessors[j - 1];
        } else {
            j -= 1;
        }
    }
    Ok(selected)
}
