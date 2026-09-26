use crate::IntervalError;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Assign intervals to the minimum number of non-overlapping lanes.
///
/// Intervals are half-open: `[start, end)`. Touching endpoints do not overlap.
/// Returns contiguous `u32` IDs from zero, in original row order. The result is
/// deterministic for identical input; no particular optimal coloring is part of
/// the contract. Ties in start order are resolved by original row index.
///
/// The minimum lane count equals maximum concurrency of non-empty intervals.
/// Empty intervals consume no capacity and receive lane zero. A nonempty input
/// consisting only of empty intervals uses one lane; empty input returns `[]`.
///
/// Sorts non-empty indices by start, then maintains the latest end of each lane
/// in a min-heap. Sorting takes `O(n log n)` time; assignment takes
/// `O(n log max(2, omega))`, where `omega` is maximum concurrency. Uses
/// `O(n + omega)` additional space, including output. Only endpoint comparisons
/// are needed, with no subtraction, graph, or endpoint-specific dependencies.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths,
/// [`IntervalError::InvalidInterval`] for the first original row with
/// `start > end`, or [`IntervalError::TooManyLanes`] if an ID exceeds `u32::MAX`.
///
/// # Examples
///
/// ```
/// use intervals_core::assign_lanes;
/// let lanes = assign_lanes(&[0, 1, 2], &[2, 3, 4])?;
/// assert_ne!(lanes[0], lanes[1]);
/// assert_ne!(lanes[1], lanes[2]);
/// assert_eq!(lanes[0], lanes[2]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn assign_lanes<T>(starts: &[T], ends: &[T]) -> Result<Vec<u32>, IntervalError>
where
    T: Ord + Copy,
{
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    let mut order = Vec::with_capacity(starts.len());
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        if start < end {
            order.push(i);
        }
    }
    order.sort_unstable_by_key(|&i| (starts[i], i));
    let mut lanes = vec![0; starts.len()];
    let mut active = BinaryHeap::<Reverse<(T, u32)>>::new();
    for i in order {
        if let Some(mut earliest) = active.peek_mut()
            && earliest.0.0 <= starts[i]
        {
            lanes[i] = earliest.0.1;
            // Replacing the heap root needs one sift, instead of pop + push.
            *earliest = Reverse((ends[i], lanes[i]));
            continue;
        }
        // All existing lanes overlap this start: a new lane is necessary.
        // The heap retains exactly one entry per lane, even after it ends.
        let lane = u32::try_from(active.len()).map_err(|_| IntervalError::TooManyLanes)?;
        lanes[i] = lane;
        active.push(Reverse((ends[i], lane)));
    }
    Ok(lanes)
}
