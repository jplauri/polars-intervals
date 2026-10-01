use crate::{
    CoverageEndpoint, CoverageStats, CoverageStatsError, geometry::merge_sorted, validate_intervals,
};

// Shared with the private offline-sweep benchmark through a #[path] import.
pub(crate) struct Prepared<T> {
    pub(crate) starts: Vec<T>,
    pub(crate) ends: Vec<T>,
    pub(crate) union: Vec<(T, T)>,
    pub(crate) prefixes: Vec<i128>,
}

impl<T: CoverageEndpoint> Prepared<T> {
    // Call only after validating every row in both operands.
    pub(crate) fn new(starts: &[T], ends: &[T]) -> Self {
        let mut records: Vec<_> = starts
            .iter()
            .zip(ends)
            .filter_map(|(&s, &e)| (s < e).then_some((s, e)))
            .collect();
        if !records.is_sorted_by_key(|r| r.0) {
            records.sort_unstable_by_key(|r| r.0);
        }
        let (starts, mut ends): (Vec<_>, Vec<_>) = records.iter().copied().unzip();
        if !ends.is_sorted() {
            ends.sort_unstable();
        }
        let union = merge_sorted(records.into_iter());
        let mut prefixes = Vec::with_capacity(union.len() + 1);
        prefixes.push(0);
        for &(start, end) in &union {
            // Disjoint runs sum to at most the coordinate span, <= u64::MAX.
            prefixes.push(prefixes.last().unwrap() + end.widened() - start.widened());
        }
        Self {
            starts,
            ends,
            union,
            prefixes,
        }
    }

    pub(crate) fn prefix_at(&self, x: T, k: usize) -> i128 {
        self.prefixes[k]
            + if k < self.union.len() && self.union[k].0 < x {
                x.widened() - self.union[k].0.widened()
            } else {
                0
            }
    }
}

/// Count original source rows and measure their union inside each query.
///
/// All intervals are half-open. Empty queries give `(0, 0, 0, None)`.
/// Empty sources have no effect. Duplicate sources increase counts independently.
/// Query rows remain separate and retain their original order. A row overlaps
/// its own source copy when both operands are the same collection.
///
/// For a nonempty `[a,b)`, every source ending at or before `a` also starts
/// before `b`. Subtracting those rows from all starts strictly before `b`
/// therefore counts exactly the overlapping original rows, including duplicates.
///
/// Source union runs are disjoint and sorted. For boundary `x`, completed runs
/// supply their cumulative length and at most one next run supplies a partial
/// length. Thus `F(b)-F(a)` measures union coverage, including gaps correctly.
/// Prefix arithmetic widens before subtraction and remains exact in `i128`.
/// The sealed endpoint domain bounds each union's length by `u64::MAX`.
/// Fractions cast each exact length to `f64` and then divide, without clamping.
///
/// Time is `O(m log(m+1) + n log(m+1) + n)` for `n` queries and `m` sources.
/// Space is `O(m+n)`, including output. Verified source order avoids its sort.
/// Verified increasing query starts AND ends use direct monotone scans when
/// `n >= m`. Otherwise, at least 32 union runs and `n <= m` select sorted query
/// boundary scans. This guard keeps query sorting within the same time bound.
/// Remaining inputs use binary searches. These are measured performance choices,
/// not assumptions about the input and not guarantees of the fastest route.
///
/// # Errors
///
/// Both original operands validate before preparation or empty shortcuts.
/// Errors identify `queries` or `intervals` and the original operand row.
/// Operand row counts may differ. Count conversions and subtraction are checked.
///
/// # Examples
///
/// ```
/// let rows = intervals_core::coverage_stats(&[0, 5, 12, 7], &[10, 10, 15, 7],
///                                           &[1, 4], &[7, 9])?;
/// assert_eq!(rows[0].overlap_count, 2);
/// assert_eq!(rows[0].covered_length, 8);
/// assert_eq!(rows[3].covered_fraction, None);
/// # Ok::<(), intervals_core::CoverageStatsError>(())
/// ```
pub fn coverage_stats<T: CoverageEndpoint>(
    query_starts: &[T],
    query_ends: &[T],
    interval_starts: &[T],
    interval_ends: &[T],
) -> Result<Vec<CoverageStats>, CoverageStatsError> {
    validate_intervals(query_starts, query_ends).map_err(CoverageStatsError::Queries)?;
    validate_intervals(interval_starts, interval_ends).map_err(CoverageStatsError::Intervals)?;
    if query_starts.is_empty() {
        return Ok(Vec::new());
    }
    let prepared = Prepared::new(interval_starts, interval_ends);
    let ordered = query_starts.len() >= interval_starts.len()
        && query_starts.is_sorted()
        && query_ends.is_sorted();
    if ordered {
        direct(&prepared, query_starts, query_ends, true)
    } else if query_starts.len() <= interval_starts.len() && prepared.union.len() >= 32 {
        sweep(&prepared, query_starts, query_ends)
    } else {
        direct(&prepared, query_starts, query_ends, false)
    }
}

pub(crate) fn direct<T: CoverageEndpoint>(
    prepared: &Prepared<T>,
    qs: &[T],
    qe: &[T],
    ordered: bool,
) -> Result<Vec<CoverageStats>, CoverageStatsError> {
    let (mut started, mut ended, mut left, mut right) = (0, 0, 0, 0);
    let mut output = Vec::with_capacity(qs.len());
    for (&a, &b) in qs.iter().zip(qe) {
        let query_length = b.widened() - a.widened();
        let (overlap_count, covered_length) = if a == b {
            (0, 0)
        } else {
            if ordered {
                while started < prepared.starts.len() && prepared.starts[started] < b {
                    started += 1;
                }
                while ended < prepared.ends.len() && prepared.ends[ended] <= a {
                    ended += 1;
                }
                while left < prepared.union.len() && prepared.union[left].1 <= a {
                    left += 1;
                }
                while right < prepared.union.len() && prepared.union[right].1 <= b {
                    right += 1;
                }
            } else {
                started = prepared.starts.partition_point(|&s| s < b);
                ended = prepared.ends.partition_point(|&e| e <= a);
                left = prepared.union.partition_point(|&(_, e)| e <= a);
                right = prepared.union.partition_point(|&(_, e)| e <= b);
            }
            let count = started
                .checked_sub(ended)
                .ok_or(CoverageStatsError::CountOverflow)?;
            (
                u64::try_from(count).map_err(|_| CoverageStatsError::CountOverflow)?,
                prepared.prefix_at(b, right) - prepared.prefix_at(a, left),
            )
        };
        output.push(CoverageStats {
            overlap_count,
            covered_length,
            query_length,
            covered_fraction: (a < b).then(|| covered_length as f64 / query_length as f64),
        });
    }
    Ok(output)
}

pub(crate) fn sweep<T: CoverageEndpoint>(
    prepared: &Prepared<T>,
    qs: &[T],
    qe: &[T],
) -> Result<Vec<CoverageStats>, CoverageStatsError> {
    let mut output: Vec<_> = qs
        .iter()
        .zip(qe)
        .map(|(&a, &b)| CoverageStats {
            overlap_count: 0,
            covered_length: 0,
            query_length: b.widened() - a.widened(),
            covered_fraction: (a < b).then_some(0.0),
        })
        .collect();
    // Packed requests keep the scan sequential. The two orders are independent:
    // start-sorted nested queries can have decreasing ends.
    let mut starts: Vec<_> = qs
        .iter()
        .zip(qe)
        .enumerate()
        .filter_map(|(i, (&a, &b))| (a < b).then_some((a, i)))
        .collect();
    let mut ends: Vec<_> = qs
        .iter()
        .zip(qe)
        .enumerate()
        .filter_map(|(i, (&a, &b))| (a < b).then_some((b, i)))
        .collect();
    if !starts.is_sorted_by_key(|r| r.0) {
        starts.sort_unstable_by_key(|r| r.0);
    }
    if !ends.is_sorted_by_key(|r| r.0) {
        ends.sort_unstable_by_key(|r| r.0);
    }
    let (mut ended, mut k) = (0, 0);
    for (a, i) in starts {
        while ended < prepared.ends.len() && prepared.ends[ended] <= a {
            ended += 1;
        }
        while k < prepared.union.len() && prepared.union[k].1 <= a {
            k += 1;
        }
        output[i].overlap_count =
            u64::try_from(ended).map_err(|_| CoverageStatsError::CountOverflow)?;
        output[i].covered_length = -prepared.prefix_at(a, k);
    }
    let (mut started, mut k) = (0, 0);
    for (b, i) in ends {
        while started < prepared.starts.len() && prepared.starts[started] < b {
            started += 1;
        }
        while k < prepared.union.len() && prepared.union[k].1 <= b {
            k += 1;
        }
        let count = u64::try_from(started).map_err(|_| CoverageStatsError::CountOverflow)?;
        let row = &mut output[i];
        row.overlap_count = count
            .checked_sub(row.overlap_count)
            .ok_or(CoverageStatsError::CountOverflow)?;
        row.covered_length += prepared.prefix_at(b, k);
        row.covered_fraction = Some(row.covered_length as f64 / row.query_length as f64);
    }
    Ok(output)
}
