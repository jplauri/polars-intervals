use crate::{IntervalError, validate_intervals};

/// Connected components of half-open intervals, in original row order.
///
/// Nonempty rows are related by strict overlap, or by overlap and touching when
/// `include_touching` is true. Every empty row is an isolated component in both
/// modes. IDs are contiguous from zero, ordered by each component's first
/// original row. Permuting rows can therefore renumber components.
///
/// A start-sorted sweep tracks the furthest end of the current component. Every
/// joining row connects to a row attaining that end. At a boundary, no earlier
/// row can connect to any later row. A final original-row pass canonicalizes IDs.
/// Only endpoint comparisons are used. Runtime is `O(n log n)` and extra space
/// is `O(n)`, including output. Verified nonempty start order takes `O(n)` time
/// and allocates only the output labels. Empty rows do not affect sortedness.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths,
/// [`IntervalError::InvalidInterval`] for the first reversed original row, or
/// [`IntervalError::TooManyClusters`] if a component ID exceeds `u32::MAX`.
///
/// # Examples
///
/// ```
/// use intervals_core::cluster_intervals;
/// assert_eq!(cluster_intervals(&[10, 0, 1], &[12, 2, 3], false)?, [0, 1, 1]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn cluster_intervals<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    include_touching: bool,
) -> Result<Vec<u32>, IntervalError> {
    validate_intervals(starts, ends)?;
    if nonempty_rows(starts, ends, None).is_sorted_by_key(|r| r.0) {
        return cluster_presorted(starts, ends, include_touching);
    }
    let nonempty = nonempty_rows(starts, ends, None).count();
    let mut records = Vec::with_capacity(nonempty);
    let mut labels = vec![0; starts.len()];
    let mut remap = Vec::<Option<u32>>::new();
    for (row, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start < end {
            records.push((start, end, row));
        } else {
            labels[row] = cluster_id(remap.len() as u64)?;
            remap.push(None);
        }
    }
    records.sort_unstable_by_key(|r| r.0);
    let mut frontier = None;
    let mut current = 0;
    for (start, end, row) in records {
        let separate = frontier.is_none_or(|right| {
            if include_touching {
                start > right
            } else {
                start >= right
            }
        });
        if separate {
            current = cluster_id(remap.len() as u64)?;
            remap.push(None);
            frontier = Some(end);
        } else {
            frontier = Some(frontier.unwrap().max(end));
        }
        labels[row] = current;
    }
    let mut next = 0;
    for label in &mut labels {
        let canonical = &mut remap[*label as usize];
        *label = match *canonical {
            Some(id) => id,
            None => {
                let id = cluster_id(next)?;
                *canonical = Some(id);
                next += 1;
                id
            }
        };
    }
    Ok(labels)
}

// Check IDs, not the component count: u32::MAX is a valid final ID.
fn cluster_id(id: u64) -> Result<u32, IntervalError> {
    u32::try_from(id).map_err(|_| IntervalError::TooManyClusters)
}

// First occurrence is already canonical in start order. Empty rows consume an
// ID in original order without resetting or extending the nonempty frontier.
fn cluster_presorted<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    touching: bool,
) -> Result<Vec<u32>, IntervalError> {
    let mut labels = Vec::with_capacity(starts.len());
    let mut frontier = None;
    let mut current = 0;
    let mut next = 0;
    for (&start, &end) in starts.iter().zip(ends) {
        if start == end {
            labels.push(cluster_id(next)?);
            next += 1;
        } else {
            if frontier.is_none_or(|right| {
                if touching {
                    start > right
                } else {
                    start >= right
                }
            }) {
                current = cluster_id(next)?;
                next += 1;
                frontier = Some(end);
            } else {
                frontier = Some(frontier.unwrap().max(end));
            }
            labels.push(current);
        }
    }
    Ok(labels)
}

/// Canonical exact union as sorted, maximal nonempty half-open ranges.
///
/// Empty input rows contribute nothing. Overlapping and touching ranges always
/// coalesce. Each output end is strictly less than the following output start.
/// The output contains endpoints only, with no labels or multiplicities.
///
/// In start order, a row starting at or before the current maximum end extends
/// that exact covered run. A larger start establishes a positive uncovered gap.
/// Only endpoint comparisons are used. Runtime is `O(n + m log m + z)` and
/// extra space is `O(m + z)`, where `m` counts nonempty input rows and `z` counts
/// output ranges. Verified nonempty start order takes `O(n + z)` time and
/// allocates only the output ranges.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths or
/// [`IntervalError::InvalidInterval`] for the first reversed original row.
///
/// # Examples
///
/// ```
/// use intervals_core::merge_intervals;
/// assert_eq!(merge_intervals(&[0, 2, 8], &[2, 4, 8])?, [(0, 4)]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn merge_intervals<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
) -> Result<Vec<(T, T)>, IntervalError> {
    validate_intervals(starts, ends)?;
    let rows = nonempty_rows(starts, ends, None);
    if rows.clone().is_sorted_by_key(|r| r.0) {
        return Ok(merge_sorted(rows));
    }
    let mut records = Vec::with_capacity(rows.clone().count());
    records.extend(rows);
    records.sort_unstable_by_key(|r| r.0);
    Ok(merge_sorted(records.into_iter()))
}

fn merge_sorted<T: Ord + Copy>(rows: impl Iterator<Item = (T, T)>) -> Vec<(T, T)> {
    let mut output: Vec<(T, T)> = Vec::new();
    for (start, end) in rows {
        if let Some(last) = output.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            output.push((start, end));
        }
    }
    output
}

/// Canonical complement of all intervals within `[domain_start, domain_end)`.
///
/// Validates every original row before clipping, including rows outside the
/// domain and rows supplied with an empty domain. Empty intervals never split
/// gaps. Empty input returns the whole nonempty domain. Equal bounds are valid
/// and produce no output. Output ranges are sorted and strictly separated.
///
/// After clipping and sorting, the cursor is the furthest covered boundary
/// already seen. A start beyond the cursor exposes exactly one uncovered gap.
/// The final cursor-to-domain-end range supplies any uncovered tail. No union
/// or label vector is materialized. Only endpoint comparisons are used.
/// Runtime is `O(n + m log m + z)` and extra space is `O(m + z)`, where `m`
/// counts clipped nonempty rows and `z` counts gaps. Verified clipped start
/// order takes `O(n + z)` time and allocates only output ranges.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths,
/// [`IntervalError::InvalidInterval`] for the first reversed original row, or
/// [`IntervalError::InvalidDomain`] for reversed domain bounds.
///
/// # Examples
///
/// ```
/// use intervals_core::interval_gaps;
/// assert_eq!(interval_gaps(&[1, 3], &[2, 5], 0, 6)?, [(0, 1), (2, 3), (5, 6)]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn interval_gaps<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    domain_start: T,
    domain_end: T,
) -> Result<Vec<(T, T)>, IntervalError> {
    validate_intervals(starts, ends)?;
    if domain_start > domain_end {
        return Err(IntervalError::InvalidDomain);
    }
    if domain_start == domain_end {
        return Ok(Vec::new());
    }
    let rows = nonempty_rows(starts, ends, Some((domain_start, domain_end)));
    if rows.clone().is_sorted_by_key(|r| r.0) {
        return Ok(gaps_sorted(rows, domain_start, domain_end));
    }
    // A narrow domain may discard most rows. Allocate only clipped survivors.
    let mut records: Vec<_> = rows.collect();
    records.sort_unstable_by_key(|r| r.0);
    Ok(gaps_sorted(records.into_iter(), domain_start, domain_end))
}

fn gaps_sorted<T: Ord + Copy>(
    rows: impl Iterator<Item = (T, T)>,
    domain_start: T,
    domain_end: T,
) -> Vec<(T, T)> {
    let mut output = Vec::new();
    let mut cursor = domain_start;
    for (start, end) in rows {
        if cursor < start {
            output.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if cursor < domain_end {
        output.push((cursor, domain_end));
    }
    output
}

// Only call after validating every original row. Clipping preserves start order
// but can also discard the rows that made the original collection unordered.
fn nonempty_rows<'a, T: Ord + Copy>(
    starts: &'a [T],
    ends: &'a [T],
    domain: Option<(T, T)>,
) -> impl Iterator<Item = (T, T)> + Clone + 'a {
    starts.iter().zip(ends).filter_map(move |(&start, &end)| {
        let (start, end) = domain.map_or((start, end), |(left, right)| {
            (start.max(left), end.min(right))
        });
        (start < end).then_some((start, end))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_cluster_ids_accept_the_maximum_id_not_only_the_count() {
        assert_eq!(cluster_id(u64::from(u32::MAX)), Ok(u32::MAX));
        assert_eq!(
            cluster_id(u64::from(u32::MAX) + 1),
            Err(IntervalError::TooManyClusters)
        );
        assert_eq!(cluster_id(u64::MAX), Err(IntervalError::TooManyClusters));
    }

    proptest::proptest! {
        #[test]
        fn checked_cluster_ids_match_the_uint32_range(id in proptest::prelude::any::<u64>()) {
            proptest::prop_assert_eq!(cluster_id(id).ok(), u32::try_from(id).ok());
        }
    }
}
