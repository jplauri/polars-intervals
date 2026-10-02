//! Polars-independent interval algorithms.
//!
//! [`overlap_counts`] counts overlaps between half-open intervals without
//! depending on Polars or any particular endpoint type.
//! [`containment_counts`] counts other intervals contained by each row.
//! [`nesting_depths`] returns the longest strict containment chain above each row.
//! [`assign_lanes`] assigns intervals to the minimum number of lanes.
//! [`assign_balanced_lanes`] constructs or improves lane row-count balance.
//! [`max_weight_non_overlapping`] selects an exact maximum-weight schedule.
//! [`max_weight_clique`] and [`max_clique`] select one exact maximum clique.
//! [`minimum_cover`] and [`minimum_cost_cover`] cover one continuous target exactly.
//! [`minimum_dominating_set`] and [`minimum_cost_dominating_set`] dominate interval vertices.
//! [`minimum_stabbing_points`] hits every discrete interval with the fewest points.
//! [`coverage_profile`] and [`weighted_coverage_profile`] return exact load segments.
//! [`cluster_intervals`], [`merge_intervals`] and [`interval_gaps`] describe interval geometry.
//! [`subtract_intervals`] and [`intersect_intervals`] combine two covered sets.

#![forbid(unsafe_code)]

use std::fmt;

mod containment;
pub use containment::containment_counts;
mod nesting;
pub use nesting::nesting_depths;
mod lanes;
pub use lanes::assign_lanes;
mod balance;
pub use balance::{
    BalanceDiagnostics, BalanceResult, BalanceStopReason, assign_balanced_lanes,
    assign_balanced_lanes_with_diagnostics,
};
mod weighted;
pub use weighted::max_weight_non_overlapping;
mod clique;
pub use clique::{max_clique, max_weight_clique};
mod capacity;
pub use capacity::max_weight_with_capacity;
mod capacity_profile;
pub use capacity_profile::max_weight_with_capacity_profile;
mod cover;
pub use cover::{minimum_cost_cover, minimum_cover};
mod domination;
pub use domination::{minimum_cost_dominating_set, minimum_dominating_set};
mod stabbing;
pub use stabbing::{DiscreteEndpoint, minimum_stabbing_points};
mod coverage;
pub use coverage::{CoverageEndpoint, max_k_coverage};
mod coverage_profile;
pub use coverage_profile::{CoverageSegment, coverage_profile, weighted_coverage_profile};
mod coverage_stats;
pub use coverage_stats::coverage_stats;
mod geometry;
pub use geometry::{cluster_intervals, interval_gaps, merge_intervals};
mod set_geometry;
pub use set_geometry::{IntervalSetError, intersect_intervals, subtract_intervals};

/// Exact source-row count and union coverage for one query interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverageStats {
    pub overlap_count: u64,
    pub covered_length: i128,
    pub query_length: i128,
    /// Integer lengths are converted separately to `f64`, then divided.
    /// Empty queries have `None`. Exact integer columns remain authoritative.
    pub covered_fraction: Option<f64>,
}

/// Invalid input to per-query coverage statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageStatsError {
    Queries(IntervalError),
    Intervals(IntervalError),
    /// A source count cannot be represented as `u64`, or count subtraction failed.
    CountOverflow,
}

impl fmt::Display for CoverageStatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queries(error) => write!(f, "queries: {error}"),
            Self::Intervals(error) => write!(f, "intervals: {error}"),
            Self::CountOverflow => write!(f, "intervals: source count exceeds the UInt64 range"),
        }
    }
}

impl std::error::Error for CoverageStatsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Queries(error) | Self::Intervals(error) => Some(error),
            Self::CountOverflow => None,
        }
    }
}

/// Invalid input to an interval algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalError {
    /// Two related slices differ in length, as `(name, length)` pairs such as
    /// `[("starts", 3), ("ends", 2)]` or `[("intervals", 3), ("weights", 2)]`.
    LengthMismatch([(&'static str, usize); 2]),
    /// An interval's start exceeds its end, at the given zero-based row index.
    InvalidInterval { index: usize },
    /// An empty interval cannot contain a stabbing point.
    EmptyInterval { index: usize },
    /// More than `u32::MAX + 1` lanes are required.
    TooManyLanes,
    /// More than `u32::MAX + 1` interval components are required.
    TooManyClusters,
    /// Lane IDs must be contiguous from zero and smaller than the row count.
    InvalidLaneId { index: usize, lane: u32 },
    /// Two nonempty intervals in the same lane overlap.
    LaneConflict { first: usize, second: usize },
    /// A supplied coloring does not use the true minimum number of lanes.
    NonMinimumLanes { actual: usize, minimum: usize },
    /// The optimal objective exceeds the `i128` accumulator range.
    WeightOverflow,
    /// A capacity-profile segment starts after it ends.
    InvalidProfileInterval { index: usize },
    /// Nonempty profile rows overlap, so their capacity is ambiguous.
    OverlappingProfile { first: usize, second: usize },
    /// Capacity must be nonnegative, including on empty profile rows.
    NegativeCapacity { index: usize },
    /// The target starts after it ends.
    InvalidTarget,
    /// No subset continuously covers the target.
    InfeasibleCover,
    /// Negative costs are unsupported, including on irrelevant intervals.
    NegativeCost { index: usize },
    /// Every feasible cover costs more than `i128::MAX`.
    CostOverflow,
    /// Coverage quantities must be nonnegative, even on irrelevant rows.
    NegativeLoad { index: usize },
    /// A positive-length coverage segment's load exceeds `i128::MAX`.
    LoadOverflow,
    /// The coverage domain starts after it ends.
    InvalidDomain,
}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch([(left, left_len), (right, right_len)]) => {
                write!(f, "lengths differ: {left_len} {left}, {right_len} {right}")
            }
            Self::InvalidInterval { index } => {
                write!(f, "interval at index {index} has start greater than end")
            }
            Self::EmptyInterval { index } => {
                write!(f, "cannot stab empty interval at index {index}")
            }
            Self::TooManyLanes => write!(f, "lane IDs exceed the UInt32 range"),
            Self::TooManyClusters => write!(f, "cluster IDs exceed the UInt32 range"),
            Self::InvalidLaneId { index, lane } => write!(
                f,
                "invalid lane ID {lane} at index {index}: IDs must be contiguous from zero and smaller than the row count"
            ),
            Self::LaneConflict { first, second } => write!(
                f,
                "intervals at indices {first} and {second} overlap in the same lane"
            ),
            Self::NonMinimumLanes { actual, minimum } => write!(
                f,
                "supplied coloring uses {actual} lanes; the minimum is {minimum}"
            ),
            Self::WeightOverflow => write!(f, "maximum weight exceeds the i128 accumulator range"),
            Self::InvalidProfileInterval { index } => write!(
                f,
                "profile interval at index {index} has start greater than end"
            ),
            Self::OverlappingProfile { first, second } => write!(
                f,
                "profile intervals at indices {first} and {second} overlap; capacity is ambiguous"
            ),
            Self::NegativeCapacity { index } => write!(
                f,
                "capacity at index {index} is negative; capacities must be nonnegative"
            ),
            Self::InvalidTarget => write!(f, "target start is greater than target end"),
            Self::InfeasibleCover => write!(
                f,
                "target interval cannot be covered by the supplied intervals"
            ),
            Self::NegativeCost { index } => write!(
                f,
                "cost at index {index} is negative; costs must be nonnegative"
            ),
            Self::CostOverflow => {
                write!(f, "minimum cover cost exceeds the i128 accumulator range")
            }
            Self::NegativeLoad { index } => write!(
                f,
                "load at index {index} is negative; loads must be nonnegative"
            ),
            Self::LoadOverflow => write!(f, "coverage load exceeds the i128 accumulator range"),
            Self::InvalidDomain => write!(f, "domain start is greater than domain end"),
        }
    }
}

impl std::error::Error for IntervalError {}

fn validate_lengths<T>(starts: &[T], ends: &[T]) -> Result<(), IntervalError> {
    check_len(("starts", starts.len()), ("ends", ends.len()))
}

fn check_len(
    left: (&'static str, usize),
    right: (&'static str, usize),
) -> Result<(), IntervalError> {
    if left.1 != right.1 {
        return Err(IntervalError::LengthMismatch([left, right]));
    }
    Ok(())
}

/// Validate matching interval slices without sorting or discarding empty rows.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal lengths or
/// [`IntervalError::InvalidInterval`] for the first original row whose start
/// exceeds its end. Empty intervals are valid.
pub fn validate_intervals<T: Ord + Copy>(starts: &[T], ends: &[T]) -> Result<(), IntervalError> {
    validate_lengths(starts, ends)?;
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
    }
    Ok(())
}

/// Counts the other intervals overlapping each interval, in input order.
///
/// Intervals are half-open: `[start, end)`. Two non-empty intervals `a` and `b`
/// overlap exactly when `a.start < b.end && b.start < a.end`. Touching endpoints
/// do not overlap. Empty intervals (`start == end`) overlap nothing and receive
/// a count of zero. Each row excludes itself, but duplicate non-empty intervals
/// are separate rows and count each other.
///
/// Sorts non-empty starts and ends, then sweeps both endpoint streams,
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
    validate_lengths(starts, ends)?;

    let mut sorted_starts = Vec::with_capacity(starts.len());
    let mut sorted_ends = Vec::with_capacity(ends.len());
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        if start < end {
            sorted_starts.push((start, index));
            sorted_ends.push((end, index));
        }
    }
    sorted_starts.sort_unstable_by_key(|&(value, _)| value);
    sorted_ends.sort_unstable_by_key(|&(value, _)| value);

    let mut counts = vec![0; starts.len()];
    let (mut started, mut ended) = (0, 0);
    while ended < sorted_ends.len() {
        // Process ends before starts at equal coordinates: touching is not overlap.
        if started < sorted_starts.len() && sorted_starts[started].0 < sorted_ends[ended].0 {
            counts[sorted_starts[started].1] = ended;
            started += 1;
        } else {
            let index = sorted_ends[ended].1;
            // At its start, save how many intervals had already ended. At its
            // end, subtract that count and this interval from all starts seen.
            counts[index] = started - counts[index] - 1;
            ended += 1;
        }
    }
    Ok(counts)
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
                Err(IntervalError::LengthMismatch([
                    ("starts", starts.len()),
                    ("ends", ends.len())
                ]))
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
