use crate::{IntervalError, geometry::merge_validated, validate_intervals};
use std::fmt;

/// An invalid operand of a two-collection set operation.
///
/// Row indices refer to the original rows within that operand. The left
/// operand validates first, followed by the right, before either is normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalSetError {
    /// The left operand is invalid.
    Left(IntervalError),
    /// The right operand is invalid.
    Right(IntervalError),
}

impl fmt::Display for IntervalSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left(error) => write!(f, "left: {error}"),
            Self::Right(error) => write!(f, "right: {error}"),
        }
    }
}

impl std::error::Error for IntervalSetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Left(error) | Self::Right(error) => error,
        })
    }
}

/// Exact set difference of two collections of half-open intervals.
///
/// Returns the covered set of the left operand minus the covered set of the
/// right operand. Empty rows contribute nothing. Duplicates and nested rows
/// do not change coverage. Output ranges are maximal, nonempty, sorted, and
/// strictly separated. Every output boundary comes from an input boundary.
///
/// Both operands validate before union normalization. The two canonical unions
/// are scanned once. A right run extending beyond a left run remains available
/// to later left runs. Only endpoint comparisons are used. For `n` left rows,
/// `m` right rows and `z` output ranges, time is `O(n log n + m log m + z)` and
/// extra space is `O(n + m + z)`. Verified start-sorted inputs take linear time.
///
/// # Errors
///
/// Returns [`IntervalSetError`] for unequal endpoint lengths within either
/// operand or its first reversed row. Left and right row counts may differ.
///
/// # Examples
///
/// ```
/// use intervals_core::subtract_intervals;
/// assert_eq!(subtract_intervals(&[0], &[10], &[2, 6], &[4, 8])?,
///            [(0, 2), (4, 6), (8, 10)]);
/// # Ok::<(), intervals_core::IntervalSetError>(())
/// ```
pub fn subtract_intervals<T: Ord + Copy>(
    left_starts: &[T],
    left_ends: &[T],
    right_starts: &[T],
    right_ends: &[T],
) -> Result<Vec<(T, T)>, IntervalSetError> {
    validate_intervals(left_starts, left_ends).map_err(IntervalSetError::Left)?;
    validate_intervals(right_starts, right_ends).map_err(IntervalSetError::Right)?;
    let left = merge_validated(left_starts, left_ends);
    if left.is_empty() {
        return Ok(left);
    }
    let right = merge_validated(right_starts, right_ends);
    let mut output = Vec::new();
    let mut j = 0;
    for (mut cursor, end) in left {
        while j < right.len() && right[j].1 <= cursor {
            j += 1;
        }
        while j < right.len() && right[j].0 < end {
            if cursor < right[j].0 {
                output.push((cursor, right[j].0));
            }
            cursor = cursor.max(right[j].1);
            if cursor >= end {
                break;
            }
            j += 1;
        }
        if cursor < end {
            output.push((cursor, end));
        }
    }
    Ok(output)
}

/// Exact geometric intersection of two half-open covered sets.
///
/// Empty rows contribute nothing and touching alone has empty intersection.
/// Duplicates and overlaps within an operand do not increase multiplicity.
/// Output ranges are maximal, nonempty, sorted, and strictly separated.
/// This enumerates covered ranges, not overlapping pairs of original rows.
///
/// Both operands validate before union normalization. A two-pointer scan
/// advances the union run with the earlier end, or both when the ends tie.
/// Only endpoint comparisons are used. For `n` left rows, `m` right rows and
/// `z` output ranges, time is `O(n log n + m log m + z)` and extra space is
/// `O(n + m + z)`. Verified start-sorted inputs take linear time.
///
/// # Errors
///
/// Returns [`IntervalSetError`] for unequal endpoint lengths within either
/// operand or its first reversed row. Left and right row counts may differ.
///
/// # Examples
///
/// ```
/// use intervals_core::intersect_intervals;
/// assert_eq!(intersect_intervals(&[0, 5], &[3, 8], &[1], &[7])?,
///            [(1, 3), (5, 7)]);
/// # Ok::<(), intervals_core::IntervalSetError>(())
/// ```
pub fn intersect_intervals<T: Ord + Copy>(
    left_starts: &[T],
    left_ends: &[T],
    right_starts: &[T],
    right_ends: &[T],
) -> Result<Vec<(T, T)>, IntervalSetError> {
    validate_intervals(left_starts, left_ends).map_err(IntervalSetError::Left)?;
    validate_intervals(right_starts, right_ends).map_err(IntervalSetError::Right)?;
    if left_starts.is_empty() || right_starts.is_empty() {
        return Ok(Vec::new());
    }
    let left = merge_validated(left_starts, left_ends);
    if left.is_empty() {
        return Ok(left);
    }
    let right = merge_validated(right_starts, right_ends);
    let mut output = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        let (a, b) = (left[i], right[j]);
        let start = a.0.max(b.0);
        let end = a.1.min(b.1);
        if start < end {
            output.push((start, end));
        }
        if a.1 <= b.1 {
            i += 1;
        }
        if b.1 <= a.1 {
            j += 1;
        }
    }
    Ok(output)
}
