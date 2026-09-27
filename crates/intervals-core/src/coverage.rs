use crate::IntervalError;

mod sealed {
    pub trait Sealed {}
}

/// An integer endpoint whose full physical span fits exactly in `i128`.
///
/// Implemented for 8/16/32/64-bit signed and unsigned integers. Use physical
/// integer days/ticks for Date/Datetime. Sealed to preserve the arithmetic bound:
/// the measure of any union is at most `u64::MAX`, even at extreme endpoints.
pub trait CoverageEndpoint: sealed::Sealed + Ord + Copy {
    fn widened(self) -> i128;
}

macro_rules! endpoints {
    ($($t:ty),*) => {$(
        impl sealed::Sealed for $t {}
        impl CoverageEndpoint for $t {
            fn widened(self) -> i128 { i128::from(self) }
        }
    )*};
}
endpoints!(i8, i16, i32, i64, u8, u16, u32, u64);

#[derive(Clone, Copy, Debug)]
struct Interval<T> {
    start: T,
    end: T,
    row: usize,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Score {
    measure: i128,
    count: usize,
}

// Independent bits: the prefix accepts its last interval; the forced-last
// state extends psi instead of the disjoint phi prefix.
const TAKE_LAST: u8 = 1;
const USE_OVERLAP: u8 = 2;

impl Score {
    fn better_than(self, other: Self) -> bool {
        self.measure > other.measure || (self.measure == other.measure && self.count < other.count)
    }

    fn add(self, measure: i128) -> Self {
        Self {
            measure: self.measure + measure,
            count: self.count + 1,
        }
    }
}

/// Select at most `k` intervals whose union has maximum total measure.
///
/// Among maximum-coverage solutions, use the fewest intervals. Returns a mask
/// in original row order, deterministic for identical input; no particular mask
/// is promised when both objectives tie. Half-open empty intervals are valid
/// and never selected. All subtraction and accumulation uses exact `i128`.
///
/// Uses the two-state offline DP of Li, Li, Duan and Lee, *Online algorithms for
/// the maximum k-interval coverage problem* (2022), DOI: 10.1007/s10878-022-00898-3.
/// The accessible preprint, arXiv:2011.10938, Section 4.1, equations (5)-(6),
/// describes the prefix/forced-last recurrence. See the repository's coverage
/// design note for the dominance proof and the at-most/minimum-count adaptation.
///
/// Worst-case time is `O(n log n + min(k,n) n)` and space is
/// `O(n + min(k,n) n)`: four rolling objective rows plus one decision byte per
/// interval per budget. Empty input and `k=0/1` take `O(n)` including validation.
/// Contained and duplicate rows are removed before DP. A minimum full-union
/// cover is returned when the budget permits it, in `O(n log n)` time.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slices, or
/// [`IntervalError::InvalidInterval`] for the first reversed original row,
/// including when `k=0`.
///
/// # Examples
///
/// ```
/// use intervals_core::max_k_coverage;
/// assert_eq!(max_k_coverage(&[0, -5, 6], &[10, 4, 15], 2)?, [false, true, true]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_k_coverage<T: CoverageEndpoint>(
    starts: &[T],
    ends: &[T],
    k: usize,
) -> Result<Vec<bool>, IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    let mut longest = None;
    let mut length = 0;
    for (row, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: row });
        }
        if k == 1 {
            let distance = end.widened() - start.widened();
            if distance > length {
                length = distance;
                longest = Some(row);
            }
        }
    }
    let mut mask = vec![false; starts.len()];
    if k <= 1 || starts.is_empty() {
        if let Some(row) = longest {
            mask[row] = true;
        }
        return Ok(mask);
    }
    let records = skyline(starts, ends);
    let full = full_cover(&records);
    if full.len() <= k {
        for i in full {
            mask[records[i].row] = true;
        }
        return Ok(mask);
    }
    drop(full);
    let phi = predecessors(&records);
    solve(&records, &phi, k, &mut mask);
    Ok(mask)
}

fn skyline<T: CoverageEndpoint>(starts: &[T], ends: &[T]) -> Vec<Interval<T>> {
    let mut records: Vec<_> = starts
        .iter()
        .zip(ends)
        .enumerate()
        .filter(|(_, (s, e))| s < e)
        .map(|(row, (&start, &end))| Interval { start, end, row })
        .collect();
    // Equal ends: smallest start last, so the reverse scan retains a container.
    // Equal geometry: smallest original row last for deterministic retention.
    records.sort_unstable_by_key(|r| (r.end, std::cmp::Reverse(r.start), std::cmp::Reverse(r.row)));
    let mut leftmost = None;
    // Reverse retain without allocating another records buffer.
    records.reverse();
    records.retain(|r| {
        if leftmost.is_none_or(|left| r.start < left) {
            leftmost = Some(r.start);
            true
        } else {
            false
        }
    });
    records.reverse();
    // Both starts and ends are now strictly increasing. Every discarded row
    // has a retained container; exchanging it preserves both optimal objectives.
    records
}

fn predecessors<T: CoverageEndpoint>(records: &[Interval<T>]) -> Vec<usize> {
    let mut p = 0;
    records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            while p < i && records[p].end <= r.start {
                p += 1;
            }
            // phi is a prefix length. If p<i, psi is index p (the leftmost start
            // among overlapping predecessors). Otherwise psi does not exist.
            p
        })
        .collect()
}

fn full_cover<T: CoverageEndpoint>(records: &[Interval<T>]) -> Vec<usize> {
    let mut chosen = Vec::new();
    let mut i = 0;
    while i < records.len() {
        chosen.push(i);
        let frontier = records[i].end;
        let mut next = i + 1;
        while next < records.len() && records[next].start <= frontier {
            next += 1;
        }
        // At a gap start the next component; otherwise choose the farthest end
        // reachable without a gap. The usual interval-cover exchange proves
        // minimum count for the complete union, component by component.
        i = if next > i + 1 { next - 1 } else { next };
    }
    chosen
}

fn solve<T: CoverageEndpoint>(
    records: &[Interval<T>],
    phi: &[usize],
    k: usize,
    mask: &mut [bool],
) -> Score {
    let n = records.len();
    let width = n + 1;
    let mut previous = vec![Score::default(); width];
    let mut forced_previous = previous.clone();
    let mut current = previous.clone();
    let mut forced = previous.clone();
    let mut decisions = Vec::with_capacity(k);
    for budget in 1..=k {
        let mut row = vec![0u8; n];
        for (i, r) in records.iter().enumerate() {
            let p = phi[i];
            let mut take = previous[p].add(r.end.widened() - r.start.widened());
            if budget > 1 && p < i {
                let overlap =
                    forced_previous[p + 1].add(r.end.widened() - records[p].end.widened());
                if overlap.better_than(take) {
                    take = overlap;
                    row[i] = USE_OVERLAP;
                }
            }
            forced[i + 1] = take;
            current[i + 1] = current[i];
            if take.better_than(current[i]) {
                current[i + 1] = take;
                row[i] |= TAKE_LAST;
            }
        }
        decisions.push(row);
        std::mem::swap(&mut previous, &mut current);
        std::mem::swap(&mut forced_previous, &mut forced);
    }
    reconstruct(records, phi, &decisions, mask);
    previous[n]
}

fn reconstruct<T>(
    records: &[Interval<T>],
    phi: &[usize],
    decisions: &[Vec<u8>],
    mask: &mut [bool],
) {
    let (mut prefix, mut budget, mut must_take) = (records.len(), decisions.len(), false);
    while prefix > 0 && budget > 0 {
        let i = prefix - 1;
        let decision = decisions[budget - 1][i];
        if !must_take && decision & TAKE_LAST == 0 {
            prefix -= 1;
            continue;
        }
        mask[records[i].row] = true;
        must_take = decision & USE_OVERLAP != 0;
        prefix = phi[i] + usize::from(must_take);
        budget -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Independent unit-cell oracle for this module's small integer generator.
    fn grid_objective(starts: &[i64], ends: &[i64], mask: &[bool]) -> Score {
        let measure = (-8..12)
            .filter(|&point| {
                starts
                    .iter()
                    .zip(ends)
                    .zip(mask)
                    .any(|((&start, &end), &selected)| selected && start <= point && point < end)
            })
            .count() as i128;
        Score {
            measure,
            count: mask.iter().filter(|&&selected| selected).count(),
        }
    }

    #[test]
    fn objective_comparison() {
        assert!(
            Score {
                measure: 2,
                count: 2
            }
            .better_than(Score {
                measure: 1,
                count: 1
            })
        );
        assert!(
            Score {
                measure: 2,
                count: 1
            }
            .better_than(Score {
                measure: 2,
                count: 2
            })
        );
        assert!(!Score::default().better_than(Score::default()));
    }

    #[test]
    fn sentinel_touching_and_overlap_helpers() {
        let r = skyline(&[0, 1, 5, 20], &[4, 5, 8, 21]);
        assert_eq!(predecessors(&r), [0, 0, 2, 3]);
    }

    proptest! {
        #[test]
        fn skyline_and_sweep_invariants(
            pairs in prop::collection::vec((-8i64..=12, -8i64..=12), 0..=30),
        ) {
            let (starts, ends): (Vec<_>, Vec<_>) = pairs.iter()
                .map(|&(a, b)| (a.min(b), a.max(b)))
                .unzip();
            let records = skyline(&starts, &ends);
            prop_assert!(records.windows(2)
                .all(|w| w[0].start < w[1].start && w[0].end < w[1].end));
            for (&start, &end) in starts.iter().zip(&ends) {
                prop_assert!(start == end || records.iter().any(|r| r.start <= start && end <= r.end));
            }
            let phi = predecessors(&records);
            for (i, interval) in records.iter().enumerate() {
                let disjoint = (0..i).filter(|&j| records[j].end <= interval.start).count();
                let psi = (0..i).find(|&j| {
                    records[j].start < interval.start && interval.start < records[j].end
                });
                prop_assert_eq!(phi[i], disjoint);
                prop_assert_eq!(psi, (phi[i] < i).then_some(phi[i]));
            }
            // Exercise the production DP directly, bypassing every fast path.
            for k in 0..=4 {
                let mut mask = vec![false; starts.len()];
                let stored = solve(&records, &phi, k, &mut mask);
                let public = max_k_coverage(&starts, &ends, k).unwrap();
                prop_assert!(mask.iter().filter(|&&selected| selected).count() <= k);
                prop_assert_eq!(stored, grid_objective(&starts, &ends, &mask));
                prop_assert_eq!(stored, grid_objective(&starts, &ends, &public));
            }
        }
    }
}
