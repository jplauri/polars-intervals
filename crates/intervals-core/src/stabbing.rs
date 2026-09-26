use crate::IntervalError;

/// A discrete ordered endpoint with an exact immediate predecessor.
///
/// Implementations must return the greatest value strictly below `self`, or
/// `None` only at the minimum value of the domain. All integer primitives implement
/// this trait. Temporal callers can use their integer physical ticks.
pub trait DiscreteEndpoint: Ord + Copy {
    fn predecessor(self) -> Option<Self>;
}

macro_rules! integers {
    ($($t:ty),*) => {$(
        impl DiscreteEndpoint for $t {
            fn predecessor(self) -> Option<Self> { self.checked_sub(1) }
        }
    )*};
}
integers!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

/// Select the minimum number of discrete points that hit every half-open interval.
///
/// Returns sorted, unique points, deterministic for identical interval geometry.
/// Empty input returns no points. The excluded right endpoint is never selected:
/// each greedy choice is the immediate predecessor of the earliest uncovered end.
///
/// # Optimality
///
/// Every feasible solution hits the earliest-ending uncovered interval with some
/// point `q`. Move `q` rightward to `predecessor(end)`. Any later-ending interval
/// hit by `q` still contains that point: its start is no greater than `q`, and its
/// end is no earlier than `end`. Thus an optimal solution contains the greedy
/// choice. Repeating on the unhit intervals proves global optimality. Equivalently,
/// the minimum stabbing number equals the maximum number of pairwise disjoint
/// intervals; the intervals triggering greedy choices form such a packing.
///
/// Uses an unstable comparison sort of packed endpoint pairs: `O(n log n)` time
/// and `O(n + k)` additional space including the `k` output points. Nondecreasing
/// ends are detected before allocation, giving `O(n)` time and `O(k)` space.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal lengths. Validation in
/// original input order returns [`IntervalError::InvalidInterval`] for `start >
/// end`, or [`IntervalError::EmptyInterval`] for `start == end`: an empty interval
/// cannot be hit. Error indices refer to the original input slices.
///
/// # Examples
///
/// ```
/// use intervals_core::minimum_stabbing_points;
/// assert_eq!(minimum_stabbing_points(&[0, 2, 5], &[4, 6, 9])?, [3, 8]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn minimum_stabbing_points<T: DiscreteEndpoint>(
    starts: &[T],
    ends: &[T],
) -> Result<Vec<T>, IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        if start == end {
            return Err(IntervalError::EmptyInterval { index });
        }
    }
    if ends.is_sorted() {
        return Ok(scan(starts.iter().copied().zip(ends.iter().copied())));
    }
    let mut intervals: Vec<_> = starts.iter().copied().zip(ends.iter().copied()).collect();
    intervals.sort_unstable_by_key(|&(_, end)| end);
    Ok(scan(intervals))
}

fn scan<T: DiscreteEndpoint>(intervals: impl IntoIterator<Item = (T, T)>) -> Vec<T> {
    let mut points = Vec::new();
    for (start, end) in intervals {
        // Ends are nondecreasing: every previous choice is already below end.
        if points.last().is_none_or(|&point| point < start) {
            // start < end proves end is not the domain minimum.
            points.push(
                end.predecessor()
                    .expect("non-empty interval has a predecessor"),
            );
        }
    }
    points
}
