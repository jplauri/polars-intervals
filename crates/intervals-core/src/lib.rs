//! Polars-independent interval algorithms.
//!
//! [`overlap_counts`] counts overlaps between half-open intervals without
//! depending on Polars or any particular endpoint type.

#![forbid(unsafe_code)]

use std::fmt;

/// Invalid input to an interval algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalError {
    /// The start and end slices have different lengths.
    LengthMismatch { starts_len: usize, ends_len: usize },
    /// An interval's start exceeds its end, at the given zero-based row index.
    InvalidInterval { index: usize },
}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                starts_len,
                ends_len,
            } => write!(
                f,
                "start and end lengths differ: {starts_len} starts, {ends_len} ends"
            ),
            Self::InvalidInterval { index } => {
                write!(f, "interval at index {index} has start greater than end")
            }
        }
    }
}

impl std::error::Error for IntervalError {}

/// Counts the other intervals overlapping each interval, in input order.
///
/// Intervals are half-open: `[start, end)`. Two non-empty intervals `a` and `b`
/// overlap exactly when `a.start < b.end && b.start < a.end`. Touching endpoints
/// do not overlap. Empty intervals (`start == end`) overlap nothing and receive
/// a count of zero. Each row excludes itself, but duplicate non-empty intervals
/// are separate rows and count each other.
///
/// Uses independently sorted non-empty starts and ends with binary searches,
/// taking `O(n log n)` time and `O(n)` additional space for `n` intervals.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] if the slices have different
/// lengths, or [`IntervalError::InvalidInterval`] for the first row where
/// `start > end`.
///
/// # Examples
///
/// ```
/// use intervals_core::overlap_counts;
///
/// let counts = overlap_counts(&[1, 3, 2, 2], &[3, 5, 4, 2])?;
/// assert_eq!(counts, vec![1, 1, 2, 0]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn overlap_counts<T>(starts: &[T], ends: &[T]) -> Result<Vec<usize>, IntervalError>
where
    T: Ord + Copy,
{
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }

    let mut sorted_starts = Vec::with_capacity(starts.len());
    let mut sorted_ends = Vec::with_capacity(ends.len());
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        if start < end {
            sorted_starts.push(start);
            sorted_ends.push(end);
        }
    }
    sorted_starts.sort_unstable();
    sorted_ends.sort_unstable();

    Ok(starts
        .iter()
        .zip(ends)
        .map(|(&start, &end)| {
            if start == end {
                return 0;
            }
            let started = sorted_starts.partition_point(|&other_start| other_start < end);
            let ended = sorted_ends.partition_point(|&other_end| other_end <= start);
            // Ended intervals are a subset of started intervals; the remainder
            // includes this non-empty interval itself.
            started - ended - 1
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{IntervalError, overlap_counts};
    use proptest::prelude::*;

    fn valid_intervals() -> impl Strategy<Value = (Vec<i32>, Vec<i32>)> {
        // Shrink collection size, starts, and lengths using built-in strategies.
        // Nonnegative lengths keep intervals valid, including during shrinking.
        let interval = (-8i32..=8, 0i32..=8).prop_map(|(start, length)| (start, start + length));
        prop::collection::vec(interval, 0..=30).prop_map(|intervals| intervals.into_iter().unzip())
    }

    fn naive_overlap_counts(starts: &[i32], ends: &[i32]) -> Vec<usize> {
        let mut counts = vec![0; starts.len()];
        for (i, count) in counts.iter_mut().enumerate() {
            for j in 0..starts.len() {
                if i != j
                    && starts[i] < ends[i]
                    && starts[j] < ends[j]
                    && starts[i] < ends[j]
                    && starts[j] < ends[i]
                {
                    *count += 1;
                }
            }
        }
        counts
    }

    fn assert_counts(starts: &[i32], ends: &[i32], expected: &[usize]) {
        assert_eq!(naive_overlap_counts(starts, ends), expected);
        assert_eq!(overlap_counts(starts, ends).unwrap(), expected);
    }

    proptest! {
        #[test]
        fn prop_matches_naive((starts, ends) in valid_intervals()) {
            prop_assert_eq!(
                overlap_counts(&starts, &ends).unwrap(),
                naive_overlap_counts(&starts, &ends)
            );
        }

        #[test]
        fn prop_preserves_output_length((starts, ends) in valid_intervals()) {
            let counts = overlap_counts(&starts, &ends).unwrap();
            prop_assert_eq!(counts.len(), starts.len());
        }

        #[test]
        fn prop_counts_are_bounded((starts, ends) in valid_intervals()) {
            let counts = overlap_counts(&starts, &ends).unwrap();
            for count in counts {
                // usize is nonnegative; a strict bound avoids n - 1 for n == 0.
                prop_assert!(count < starts.len());
            }
        }

        #[test]
        fn prop_empty_intervals_have_zero_count((starts, ends) in valid_intervals()) {
            let counts = overlap_counts(&starts, &ends).unwrap();
            for ((start, end), count) in starts.iter().zip(&ends).zip(counts) {
                if start == end {
                    prop_assert_eq!(count, 0);
                }
            }
        }

        #[test]
        fn prop_total_count_is_even((starts, ends) in valid_intervals()) {
            let counts = overlap_counts(&starts, &ends).unwrap();
            prop_assert_eq!(counts.iter().sum::<usize>() % 2, 0);
        }

        #[test]
        fn prop_translation_preserves_counts(
            (starts, ends) in valid_intervals(),
            delta in -100i32..=100,
        ) {
            // Endpoints are in [-8, 16], so shifted values stay in [-108, 116].
            let shifted_starts: Vec<_> = starts.iter().map(|&start| start + delta).collect();
            let shifted_ends: Vec<_> = ends.iter().map(|&end| end + delta).collect();
            prop_assert_eq!(
                overlap_counts(&shifted_starts, &shifted_ends).unwrap(),
                overlap_counts(&starts, &ends).unwrap()
            );
        }
    }

    #[test]
    fn empty_input() {
        assert_counts(&[], &[], &[]);
    }

    #[test]
    fn one_interval() {
        assert_counts(&[1], &[3], &[0]);
    }

    #[test]
    fn disjoint_intervals() {
        assert_counts(&[1, 4, 7], &[2, 5, 8], &[0, 0, 0]);
    }

    #[test]
    fn touching_intervals() {
        assert_counts(&[1, 3, 5], &[3, 5, 7], &[0, 0, 0]);
    }

    #[test]
    fn partially_overlapping_intervals() {
        assert_counts(&[1, 3, 5], &[4, 6, 8], &[1, 2, 1]);
    }

    #[test]
    fn nested_intervals() {
        assert_counts(&[0, 1, 2, 6], &[10, 5, 3, 9], &[3, 2, 2, 1]);
    }

    #[test]
    fn identical_intervals() {
        assert_counts(&[1, 1, 1], &[4, 4, 4], &[2, 2, 2]);
    }

    #[test]
    fn shared_starts() {
        assert_counts(&[1, 1, 1, 3], &[2, 4, 6, 5], &[2, 3, 3, 2]);
    }

    #[test]
    fn shared_ends() {
        assert_counts(&[1, 3, 5, 0], &[6, 6, 6, 2], &[3, 2, 2, 1]);
    }

    #[test]
    fn empty_intervals() {
        assert_counts(&[1, 1, 3], &[1, 1, 3], &[0, 0, 0]);
    }

    #[test]
    fn empty_intervals_inside_and_at_boundaries() {
        assert_counts(&[0, 0, 2, 4, 1], &[4, 0, 2, 4, 3], &[1, 0, 0, 0, 1]);
    }

    #[test]
    fn preserves_input_order() {
        assert_counts(&[8, 3, 0, 1], &[9, 5, 10, 2], &[1, 1, 3, 1]);
    }

    #[test]
    fn extreme_endpoints() {
        assert_counts(
            &[i32::MIN, i32::MIN, 0, i32::MAX],
            &[i32::MAX, 0, i32::MAX, i32::MAX],
            &[2, 1, 1, 0],
        );
    }

    #[test]
    fn nonnumeric_endpoints() {
        assert_eq!(
            overlap_counts(&['a', 'c', 'd'], &['d', 'f', 'd']),
            Ok(vec![1, 1, 0])
        );
    }

    #[test]
    fn rejects_invalid_intervals() {
        assert_eq!(
            overlap_counts(&[2], &[1]),
            Err(IntervalError::InvalidInterval { index: 0 })
        );
        assert_eq!(
            overlap_counts(&[0, 3, 4], &[0, 2, 1]),
            Err(IntervalError::InvalidInterval { index: 1 })
        );
    }

    #[test]
    fn rejects_mismatched_lengths() {
        for (starts, ends) in [(&[1][..], &[][..]), (&[][..], &[1][..])] {
            assert_eq!(
                overlap_counts(starts, ends),
                Err(IntervalError::LengthMismatch {
                    starts_len: starts.len(),
                    ends_len: ends.len(),
                })
            );
        }
    }

    #[test]
    fn exhaustive_small_collections_match_naive() {
        let intervals: Vec<_> = (-1..=2)
            .flat_map(|start| (start..=2).map(move |end| (start, end)))
            .collect();

        // Enumerate every ordered collection of up to five intervals, allowing
        // repeats: 1 + 10 + 100 + 1,000 + 10,000 + 100,000 = 111,111 cases.
        for len in 0..=5 {
            for mut code in 0..intervals.len().pow(len) {
                let mut starts = Vec::new();
                let mut ends = Vec::new();
                for _ in 0..len {
                    let (start, end) = intervals[code % intervals.len()];
                    code /= intervals.len();
                    starts.push(start);
                    ends.push(end);
                }
                assert_eq!(
                    overlap_counts(&starts, &ends).unwrap(),
                    naive_overlap_counts(&starts, &ends),
                    "starts={starts:?}, ends={ends:?}"
                );
            }
        }
    }
}
