use crate::{IntervalError, validate_lengths};

/// Return the length of the longest strict containment chain above each interval.
///
/// An outermost interval has depth zero. Interval `a` strictly contains `b` iff
/// `a.start <= b.start && b.end <= a.end` and at least one inequality is strict.
/// Identical geometries receive the same depth and never extend one another's
/// chains. Empty intervals use this endpoint predicate too: `[0, 5)` strictly
/// contains `[5, 5)`, while two `[5, 5)` intervals do not strictly contain each
/// other. Results are deterministic and preserve the original row order.
///
/// Packed records are ordered by ascending start, descending end, then original
/// index. A monotone frontier stores the greatest achievable last end for each
/// chain length. Exact duplicate geometries are queried before their shared
/// update; different ends at the same start can chain. Binary search locates the
/// longest extendable chain, with a constant-time append when depth increases.
/// Time is `O(n log n)` overall, including `O(n log(d + 2))` for the frontier
/// sweep at maximum depth `d`. Additional space is `O(n)` for records and output
/// plus `O(d + 1)` for the frontier. No endpoint arithmetic, coordinate
/// compression, or containment pairs are needed.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths, or
/// [`IntervalError::InvalidInterval`] for the first original row with `start > end`.
///
/// # Examples
///
/// ```
/// use intervals_core::nesting_depths;
/// assert_eq!(nesting_depths(&[0, 2, 4], &[10, 8, 6])?, [0, 1, 2]);
/// assert_eq!(nesting_depths(&[1, 1, 1], &[10, 10, 8])?, [0, 0, 1]);
/// assert_eq!(nesting_depths(&[0, 5, 5], &[5, 5, 5])?, [0, 1, 1]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn nesting_depths<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
) -> Result<Vec<usize>, IntervalError> {
    validate_lengths(starts, ends)?;
    let mut records = Vec::with_capacity(starts.len());
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        records.push((start, end, index));
    }
    records.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut depths = vec![0; starts.len()];
    let mut tails = Vec::new();
    for group in records.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
        assign_group(group, &mut tails, &mut depths);
    }
    Ok(depths)
}

fn assign_group<T: Ord + Copy>(group: &[(T, T, usize)], tails: &mut Vec<T>, depths: &mut [usize]) {
    let end = group[0].1;
    // tails[k] is the greatest last end of a chain of cardinality k+1.
    // It is nonincreasing: any chain's shorter prefix ends at least as far
    // right. The eligible prefix therefore gives the longest extendable chain.
    let depth = if tails.last().is_none_or(|&tail| tail >= end) {
        tails.len()
    } else {
        tails.partition_point(|&tail| tail >= end)
    };
    // Exact duplicates share one predecessor set. Assign every row before the
    // single update, so they can never extend one another's chains.
    for &(_, _, index) in group {
        depths[index] = depth;
    }
    if depth == tails.len() {
        tails.push(end);
    } else {
        // Shorter tails already end >= end; only this cardinality improves.
        tails[depth] = end;
    }
}

#[cfg(test)]
mod tests {
    use super::{assign_group, nesting_depths};
    use proptest::prelude::*;

    #[test]
    fn empty_frontier_assigns_depth_zero() {
        let mut tails = vec![];
        let mut depths = vec![usize::MAX];
        assign_group(&[(0, 10, 0)], &mut tails, &mut depths);
        assert_eq!(depths, [0]);
        assert_eq!(tails, [10]);
    }

    #[test]
    fn frontier_can_improve_a_shorter_chain_without_extending_maximum_depth() {
        let mut tails = vec![10, 8, 6];
        let mut depths = vec![usize::MAX; 2];
        assign_group(&[(3, 9, 0)], &mut tails, &mut depths);
        assert_eq!(depths[0], 1);
        assert_eq!(tails, [10, 9, 6]);
        assign_group(&[(4, 7, 1)], &mut tails, &mut depths);
        assert_eq!(depths[1], 2);
        assert_eq!(tails, [10, 9, 7]);
    }

    #[test]
    fn equal_end_from_later_start_appends_to_frontier() {
        let mut tails = vec![10, 10];
        let mut depths = vec![usize::MAX; 2];
        assign_group(&[(2, 10, 0), (2, 10, 1)], &mut tails, &mut depths);
        assert_eq!(depths, [2, 2]);
        assert_eq!(tails, [10, 10, 10]);
    }

    #[test]
    fn incomparable_interval_replaces_first_tail_at_depth_zero() {
        let mut tails = vec![10, 8, 6];
        let mut depths = vec![usize::MAX];
        assign_group(&[(5, 11, 0)], &mut tails, &mut depths);
        assert_eq!(depths, [0]);
        assert_eq!(tails, [11, 8, 6]);
    }

    #[test]
    fn exact_geometry_batch_is_atomic_but_same_start_different_end_is_visible() {
        assert_eq!(
            nesting_depths(&[1, 1, 1, 1, 1], &[10, 10, 8, 8, 5]).unwrap(),
            [0, 0, 1, 1, 2]
        );
    }

    #[test]
    fn same_end_new_start_is_visible_after_exact_duplicate_batch() {
        assert_eq!(
            nesting_depths(&[0, 0, 2, 2, 5], &[10, 10, 10, 10, 10]).unwrap(),
            [0, 0, 1, 1, 2]
        );
    }

    proptest! {
        #[test]
        fn frontier_invariant_matches_independent_dp_after_every_geometry_batch(
            pairs in prop::collection::vec((-8i32..=8, -8i32..=8), 0..=25),
        ) {
            let mut records: Vec<_> = pairs.into_iter().enumerate()
                .map(|(i, (a, b))| (a.min(b), a.max(b), i)).collect();
            records.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
            let mut tails = Vec::new();
            let mut depths = vec![0; records.len()];
            let mut processed = 0;
            for group in records.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
                assign_group(group, &mut tails, &mut depths);
                processed += group.len();
                let prefix = &records[..processed];
                let mut expected = vec![0usize; processed];
                for (j, &(start, end, index)) in prefix.iter().enumerate() {
                    for (i, &(outer_start, outer_end, _)) in prefix[..j].iter().enumerate() {
                        if outer_start <= start && end <= outer_end
                            && (outer_start < start || end < outer_end)
                        {
                            expected[j] = expected[j].max(expected[i] + 1);
                        }
                    }
                    prop_assert_eq!(depths[index], expected[j]);
                }
                // A row of depth >= k ends a chain of cardinality k+1: take
                // that many final rows of one of its longest chains.
                let expected_tails: Vec<_> = (0..=*expected.iter().max().unwrap())
                    .map(|k| prefix.iter().zip(&expected)
                        .filter(|(_, depth)| **depth >= k)
                        .map(|(row, _)| row.1).max().unwrap())
                    .collect();
                prop_assert_eq!(&tails, &expected_tails);
                prop_assert!(tails.windows(2).all(|pair| pair[0] >= pair[1]));
            }
        }
    }
}
