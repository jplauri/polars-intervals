use crate::{IntervalError, validate_lengths};

/// Select one maximum-cardinality clique of half-open intervals.
///
/// This is [`max_weight_clique`] with implicit unit weights, without allocating
/// a weight vector. Empty intervals are isolated vertices: an empty-only input
/// selects its first row, and empty input returns an empty mask.
///
/// Ties choose the earliest maximizing nonempty coordinate, preferring a
/// nonempty clique to an empty singleton. These tied masks are not a
/// cross-release stability promise. Sorting and reconstruction take
/// `O(n log n)` time and `O(n)` extra space, including the output. Inputs whose
/// nonempty intervals all share a point take `O(n)` time with only output space.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal lengths or
/// [`IntervalError::InvalidInterval`] for the first reversed original row.
/// Every row is validated before optimization.
///
/// # Examples
///
/// ```
/// use intervals_core::max_clique;
/// assert_eq!(max_clique(&[0, 1, 2], &[2, 3, 4])?, [true, true, false]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_clique<T>(starts: &[T], ends: &[T]) -> Result<Vec<bool>, IntervalError>
where
    T: Ord + Copy,
{
    validate_lengths(starts, ends)?;
    let mut count = 0;
    let mut common = None;
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        if start < end {
            count += 1;
            update_intersection(&mut common, start, end);
        }
    }
    if count == 0 {
        return Ok((0..starts.len()).map(|i| i == 0).collect());
    }
    if common.is_some_and(|(left, right)| left < right) {
        // At least one nonempty row exists, so this also beats or ties (and
        // takes precedence over) every isolated unit-weight singleton.
        return Ok(starts.iter().zip(ends).map(|(s, e)| s < e).collect());
    }

    // Endpoint-only streams keep the unit path free of weight/index payloads.
    let mut arrivals = Vec::with_capacity(count);
    let mut departures = Vec::with_capacity(count);
    for (&start, &end) in starts.iter().zip(ends) {
        if start < end {
            arrivals.push(start);
            departures.push(end);
        }
    }
    arrivals.sort_unstable();
    departures.sort_unstable();
    let (mut begun, mut ended) = (0, 0);
    let (mut active, mut best) = (0i128, 0i128);
    let mut coordinate = arrivals[0];
    while begun < arrivals.len() {
        let t = arrivals[begun];
        while ended < departures.len() && departures[ended] <= t {
            active = active.checked_sub(1).ok_or(IntervalError::WeightOverflow)?;
            ended += 1;
        }
        while begun < arrivals.len() && arrivals[begun] == t {
            active = active.checked_add(1).ok_or(IntervalError::WeightOverflow)?;
            begun += 1;
        }
        if active > best {
            best = active;
            coordinate = t;
        }
    }
    drop(arrivals);
    drop(departures);
    // Save only the winning coordinate; repeated improvements never clone an
    // active set. Membership also excludes every empty interval automatically.
    Ok(starts
        .iter()
        .zip(ends)
        .map(|(&start, &end)| start <= coordinate && coordinate < end)
        .collect())
}

/// Select one globally maximum-weight clique of half-open intervals.
///
/// Returns a Boolean mask in original row order. Every selected pair must
/// overlap: touching intervals are incompatible. Duplicate nonempty intervals
/// are distinct vertices whose positive weights accumulate. Empty intervals
/// are isolated vertices and may only be selected as singletons.
///
/// The empty clique has objective zero; zero and negative weights are omitted.
/// Weights convert losslessly to `i128`, with checked objective arithmetic.
/// Nonempty cliques tie at their earliest maximizing coordinate and beat
/// equal-weight empty singletons. Equal empty singletons choose the lowest
/// original row index. Tied masks are not a cross-release stability promise.
///
/// A pairwise-intersecting set of nonempty intervals shares a point: its
/// greatest start is less than its least end. After removing nonpositive
/// weights, an optimum therefore consists of all positive intervals covering
/// some point. Its weight can increase only at a start coordinate. Two sorted
/// endpoint streams evaluate those coordinates, expiring ends before adding
/// equal starts. Comparing the best state with the best isolated singleton
/// and zero covers every possible clique. One final pass reconstructs the mask.
///
/// Takes `O(n log n)` time and `O(n)` extra space, including sorting and output.
/// When all positive nonempty intervals share a point, validation detects that
/// clique and takes an `O(n)` path requiring only output space.
/// Endpoints need only ordering and copying, never arithmetic. Use
/// [`max_clique`] for implicit unit weights without a weight-vector allocation.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal lengths,
/// [`IntervalError::InvalidInterval`] for the first reversed original row
/// (even if its weight is nonpositive), or [`IntervalError::WeightOverflow`]
/// if a clique objective cannot be represented in `i128`. All rows are
/// validated before optimization; unrelated interval weights are never summed.
///
/// # Examples
///
/// ```
/// use intervals_core::max_weight_clique;
/// assert_eq!(
///     max_weight_clique(&[0, 1, 2], &[2, 3, 4], &[4, 5, 6])?,
///     [false, true, true],
/// );
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_weight_clique<T, W>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
) -> Result<Vec<bool>, IntervalError>
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
    let (mut count, mut singleton) = (0, None);
    let mut singleton_weight = 0i128;
    let mut common = None;
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        let weight = i128::from(weights[index]);
        if weight > 0 {
            if start < end {
                count += 1;
                update_intersection(&mut common, start, end);
            } else if weight > singleton_weight {
                singleton = Some(index);
                singleton_weight = weight;
            }
        }
    }
    if count == 0 {
        return Ok((0..starts.len()).map(|i| singleton == Some(i)).collect());
    }
    if common.is_some_and(|(left, right)| left < right) {
        // Only sum after validation and the proof that every useful interval
        // belongs to this one clique. Disjoint i128::MAX rows must not overflow.
        let mut total = 0i128;
        for ((&start, &end), &weight) in starts.iter().zip(ends).zip(weights) {
            let weight = i128::from(weight);
            if start < end && weight > 0 {
                total = total
                    .checked_add(weight)
                    .ok_or(IntervalError::WeightOverflow)?;
            }
        }
        return Ok(starts
            .iter()
            .zip(ends)
            .enumerate()
            .map(|(i, (&start, &end))| {
                if singleton_weight > total {
                    singleton == Some(i)
                } else {
                    start < end && i128::from(weights[i]) > 0
                }
            })
            .collect());
    }

    let mut arrivals = Vec::with_capacity(count);
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start < end && i128::from(weights[i]) > 0 {
            arrivals.push(i);
        }
    }
    let mut departures = arrivals.clone();
    arrivals.sort_unstable_by_key(|&i| starts[i]);
    departures.sort_unstable_by_key(|&i| ends[i]);
    let (mut begun, mut ended) = (0, 0);
    let (mut active, mut best) = (0i128, 0i128);
    let mut coordinate = starts[arrivals[0]];
    while begun < arrivals.len() {
        let t = starts[arrivals[begun]];
        while ended < departures.len() && ends[departures[ended]] <= t {
            active = active
                .checked_sub(i128::from(weights[departures[ended]]))
                .ok_or(IntervalError::WeightOverflow)?;
            ended += 1;
        }
        while begun < arrivals.len() && starts[arrivals[begun]] == t {
            active = active
                .checked_add(i128::from(weights[arrivals[begun]]))
                .ok_or(IntervalError::WeightOverflow)?;
            begun += 1;
        }
        if active > best {
            best = active;
            coordinate = t;
        }
    }
    drop(arrivals);
    drop(departures);
    Ok(starts
        .iter()
        .zip(ends)
        .enumerate()
        .map(|(i, (&start, &end))| {
            if singleton_weight > best {
                singleton == Some(i)
            } else {
                i128::from(weights[i]) > 0 && start <= coordinate && coordinate < end
            }
        })
        .collect())
}

fn update_intersection<T: Ord + Copy>(intersection: &mut Option<(T, T)>, start: T, end: T) {
    match intersection {
        None => *intersection = Some((start, end)),
        Some((left, right)) if *left < *right => {
            *left = (*left).max(start);
            *right = (*right).min(end);
        }
        // Further rows cannot restore an intersection once it becomes empty.
        _ => {}
    }
}
