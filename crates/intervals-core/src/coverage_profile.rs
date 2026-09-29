use crate::{IntervalError, validate_lengths};

/// One maximal constant-load half-open segment of a coverage profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageSegment<T> {
    /// Inclusive left endpoint.
    pub start: T,
    /// Exclusive right endpoint, strictly greater than `start`.
    pub end: T,
    /// Exact nonnegative load throughout `[start, end)`.
    pub load: i128,
}

/// Compute the exact number of active half-open intervals as canonical segments.
///
/// Empty intervals contribute nothing. Touching segments with equal loads are
/// coalesced, even when their active rows differ. Without `domain`, the domain
/// is the hull of all nonempty intervals; empty-only inputs have no domain.
/// An explicit domain clips contributions after every input row is validated.
/// `include_zero` includes gaps and zero tails within that domain only.
///
/// Two independently sorted endpoint streams use `O(n + m log m + z)` time
/// and `O(m + z)` additional space, where `m` is the number of clipped nonempty
/// intervals and `z` the number of output segments. Verified ordered streams
/// take `O(n + z)` time. Unit loads require no weight vector. Endpoints need
/// only order comparisons, without subtraction or coordinate conversion.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal lengths,
/// [`IntervalError::InvalidInterval`] for the first reversed original row,
/// [`IntervalError::InvalidDomain`] for reversed bounds, or
/// [`IntervalError::LoadOverflow`] for an unrepresentable represented load.
///
/// # Examples
///
/// ```
/// use intervals_core::{CoverageSegment, coverage_profile};
/// assert_eq!(coverage_profile(&[0, 2], &[2, 4], None, false)?,
///     [CoverageSegment { start: 0, end: 4, load: 1 }]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn coverage_profile<T>(
    starts: &[T],
    ends: &[T],
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    T: Ord + Copy,
{
    let (domain, count) = validate(starts, ends, domain, |_| 1)?;
    let Some((left, right)) = domain.filter(|(left, right)| left < right) else {
        return Ok(Vec::new());
    };
    let mut arrivals = Vec::with_capacity(count);
    let mut departures = Vec::with_capacity(count);
    for (&start, &end) in starts.iter().zip(ends) {
        let (start, end) = (start.max(left), end.min(right));
        if start < end {
            arrivals.push(start);
            departures.push(end);
        }
    }
    if !arrivals.is_sorted() {
        arrivals.sort_unstable();
    }
    if !departures.is_sorted() {
        departures.sort_unstable();
    }
    sweep(
        arrivals.into_iter().map(|t| (t, 1)),
        departures.into_iter().map(|t| (t, 1)),
        (left, right),
        include_zero,
    )
}

/// Compute the exact sum of active nonnegative quantities as canonical segments.
///
/// Quantities are constant per interval, not divided by duration. Zero-weight
/// nonempty rows establish or enlarge the inferred domain, but create no load
/// or breakpoints. Empty rows never affect the domain. Otherwise the domain,
/// coalescing and zero-gap rules are those of [`coverage_profile`].
///
/// Uses two independently sorted `(endpoint, quantity)` streams, retaining the
/// input quantity type in their records. With `m` positive clipped nonempty rows
/// and `z` output segments,
/// takes `O(n + m log m + z)` time and `O(m + z)` additional space. Verified
/// ordered streams take `O(n + z)` time. Loads are accumulated directly with
/// checked `i128` arithmetic; neither total input weight nor load-area is summed.
///
/// # Errors
///
/// In addition to the errors documented by [`coverage_profile`], returns
/// [`IntervalError::NegativeLoad`] for a negative original row, including empty,
/// clipped-away, and zero-domain inputs. `W` must convert losslessly to `i128`.
/// Overflow is an error only on positive-length segments inside the domain.
pub fn weighted_coverage_profile<T, W>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    T: Ord + Copy,
    W: Copy,
    i128: From<W>,
{
    validate_lengths(starts, ends)?;
    if starts.len() != weights.len() {
        return Err(IntervalError::LengthMismatch([
            ("intervals", starts.len()),
            ("weights", weights.len()),
        ]));
    }
    let (domain, count) = validate(starts, ends, domain, |i| i128::from(weights[i]))?;
    let Some((left, right)) = domain.filter(|(left, right)| left < right) else {
        return Ok(Vec::new());
    };
    let mut arrivals = Vec::with_capacity(count);
    let mut departures = Vec::with_capacity(count);
    for ((&start, &end), &weight) in starts.iter().zip(ends).zip(weights) {
        let (start, end) = (start.max(left), end.min(right));
        if start < end && i128::from(weight) > 0 {
            arrivals.push((start, weight));
            departures.push((end, weight));
        }
    }
    // Natural-alignment records keep comparisons and sweep reads contiguous.
    // Their extra storage trades off against index streams' random source reads.
    if !arrivals.is_sorted_by_key(|&(coordinate, _)| coordinate) {
        arrivals.sort_unstable_by_key(|&(coordinate, _)| coordinate);
    }
    if !departures.is_sorted_by_key(|&(coordinate, _)| coordinate) {
        departures.sort_unstable_by_key(|&(coordinate, _)| coordinate);
    }
    sweep(
        arrivals
            .into_iter()
            .map(|(coordinate, weight)| (coordinate, i128::from(weight))),
        departures
            .into_iter()
            .map(|(coordinate, weight)| (coordinate, i128::from(weight))),
        (left, right),
        include_zero,
    )
}

// Count only represented positive contributions, but validate every original
// row and infer the hull from ALL nonempty rows (including zero quantities).
fn validate<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    domain: Option<(T, T)>,
    weight: impl Fn(usize) -> i128,
) -> Result<(Option<(T, T)>, usize), IntervalError> {
    validate_lengths(starts, ends)?;
    if domain.is_some_and(|(left, right)| left > right) {
        return Err(IntervalError::InvalidDomain);
    }
    let mut hull = None;
    let mut count = 0;
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        let quantity = weight(index);
        if quantity < 0 {
            return Err(IntervalError::NegativeLoad { index });
        }
        if start < end {
            hull = Some(hull.map_or((start, end), |(left, right): (T, T)| {
                (left.min(start), right.max(end))
            }));
            if quantity > 0 && domain.is_none_or(|(left, right)| start.max(left) < end.min(right)) {
                count += 1;
            }
        }
    }
    Ok((domain.or(hull), count))
}

fn sweep<T: Ord + Copy>(
    arrivals: impl Iterator<Item = (T, i128)>,
    departures: impl Iterator<Item = (T, i128)>,
    (left, right): (T, T),
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError> {
    let mut arrivals = arrivals.peekable();
    let mut departures = departures.peekable();
    let mut output: Vec<CoverageSegment<T>> = Vec::new();
    let (mut previous, mut load) = (left, 0i128);
    loop {
        let coordinate = match (arrivals.peek(), departures.peek()) {
            (Some(&(a, _)), Some(&(b, _))) => a.min(b),
            (Some(&(a, _)), None) => a,
            (None, Some(&(b, _))) => b,
            (None, None) => right,
        };
        if previous < coordinate && (include_zero || load > 0) {
            if let Some(last) = output
                .last_mut()
                .filter(|last| last.end == previous && last.load == load)
            {
                last.end = coordinate;
            } else {
                output.push(CoverageSegment {
                    start: previous,
                    end: coordinate,
                    load,
                });
            }
        }
        if coordinate == right {
            return Ok(output);
        }
        // All departures precede all arrivals at a tied coordinate. Intermediate
        // arrival sums increase monotonically to the load of the next cell, so
        // checked arithmetic cannot falsely overflow at a zero-time transient.
        while let Some((_, quantity)) = departures.next_if(|&(t, _)| t == coordinate) {
            load = load
                .checked_sub(quantity)
                .ok_or(IntervalError::LoadOverflow)?;
        }
        while let Some((_, quantity)) = arrivals.next_if(|&(t, _)| t == coordinate) {
            load = load
                .checked_add(quantity)
                .ok_or(IntervalError::LoadOverflow)?;
        }
        previous = coordinate;
    }
}
