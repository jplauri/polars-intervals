//! Private full-profile experiments with local validation/emission helpers.
//! Correctness comes from the independent direct-membership oracle below;
//! agreement between related sweep implementations is only a cross-check.
use std::{cmp::Reverse, collections::BinaryHeap};

use intervals_core::{CoverageSegment, IntervalError, coverage_profile, weighted_coverage_profile};

pub const METHODS: &[&str] = &["production", "events", "heap", "streams_indices"];

pub fn run<T: Ord + Copy, W: Copy>(
    method: &str,
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    i128: From<W>,
{
    match method {
        "production" => weights.map_or_else(
            || coverage_profile(starts, ends, domain, include_zero),
            |weights| weighted_coverage_profile(starts, ends, weights, domain, include_zero),
        ),
        "events" => events(starts, ends, weights, domain, include_zero),
        "heap" => heap(starts, ends, weights, domain, include_zero),
        "streams_indices" => match weights {
            Some(weights) => indices(starts, ends, weights, domain, include_zero),
            // This comparison only concerns weighted representation. The harness
            // does not measure it in unit mode, where production is endpoint-only.
            None => coverage_profile(starts, ends, domain, include_zero),
        },
        _ => panic!("unknown method {method}"),
    }
}

fn quantity<W: Copy>(weights: Option<&[W]>, row: usize) -> i128
where
    i128: From<W>,
{
    weights.map_or(1, |weights| i128::from(weights[row]))
}

fn validate<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
    domain: Option<(T, T)>,
) -> Result<(Option<(T, T)>, usize), IntervalError>
where
    i128: From<W>,
{
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch([
            ("starts", starts.len()),
            ("ends", ends.len()),
        ]));
    }
    if let Some(weights) = weights
        && weights.len() != starts.len()
    {
        return Err(IntervalError::LengthMismatch([
            ("intervals", starts.len()),
            ("weights", weights.len()),
        ]));
    }
    if let Some((lo, hi)) = domain
        && lo > hi
    {
        return Err(IntervalError::InvalidDomain);
    }
    let mut hull = None;
    let mut count = 0;
    for row in 0..starts.len() {
        if starts[row] > ends[row] {
            return Err(IntervalError::InvalidInterval { index: row });
        }
        if quantity(weights, row) < 0 {
            return Err(IntervalError::NegativeLoad { index: row });
        }
        if starts[row] < ends[row] {
            hull = Some(hull.map_or((starts[row], ends[row]), |(lo, hi): (T, T)| {
                (lo.min(starts[row]), hi.max(ends[row]))
            }));
            if quantity(weights, row) > 0
                && domain.is_none_or(|(lo, hi)| starts[row].max(lo) < ends[row].min(hi))
            {
                count += 1;
            }
        }
    }
    Ok((domain.or(hull).filter(|(lo, hi)| lo < hi), count))
}

fn emit<T: Ord + Copy>(
    output: &mut Vec<CoverageSegment<T>>,
    start: T,
    end: T,
    load: i128,
    include_zero: bool,
) {
    if start >= end || (!include_zero && load == 0) {
        return;
    }
    if let Some(previous) = output.last_mut()
        && previous.end == start
        && previous.load == load
    {
        previous.end = end;
    } else {
        output.push(CoverageSegment { start, end, load });
    }
}

// Coordinate plus a tagged row index: for i64/u64, 16 bytes at 8-byte alignment.
// The low bit is 0 for departure and 1 for arrival. Payloads stay in caller data.
fn events<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    i128: From<W>,
{
    let (domain, count) = validate(starts, ends, weights, domain)?;
    let Some((lo, hi)) = domain else {
        return Ok(Vec::new());
    };
    let mut events = Vec::with_capacity(2 * count);
    for row in 0..starts.len() {
        let (start, end) = (starts[row].max(lo), ends[row].min(hi));
        if start < end && quantity(weights, row) > 0 {
            events.push((start, 2 * row + 1));
            events.push((end, 2 * row));
        }
    }
    events.sort_unstable_by_key(|&(coordinate, tag)| (coordinate, tag % 2));
    let mut output = Vec::new();
    let (mut previous, mut load) = (lo, 0i128);
    for (coordinate, tag) in events {
        emit(&mut output, previous, coordinate, load, include_zero);
        let weight = quantity(weights, tag / 2);
        load = if tag % 2 == 0 {
            load.checked_sub(weight)
        } else {
            load.checked_add(weight)
        }
        .ok_or(IntervalError::LoadOverflow)?;
        previous = coordinate;
    }
    emit(&mut output, previous, hi, load, include_zero);
    Ok(output)
}

fn heap<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    i128: From<W>,
{
    let (domain, count) = validate(starts, ends, weights, domain)?;
    let Some((lo, hi)) = domain else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::with_capacity(count);
    rows.extend(
        (0..starts.len())
            .filter(|&row| starts[row].max(lo) < ends[row].min(hi) && quantity(weights, row) > 0),
    );
    if !rows
        .windows(2)
        .all(|pair| starts[pair[0]] <= starts[pair[1]])
    {
        rows.sort_unstable_by_key(|&row| starts[row]);
    }
    // Deliberately do not reserve n: this buffer should follow active concurrency.
    let mut active = BinaryHeap::<Reverse<(T, usize)>>::new();
    let mut output = Vec::new();
    let (mut cursor, mut previous, mut load) = (0, lo, 0i128);
    while cursor < rows.len() || !active.is_empty() {
        let next_start = rows.get(cursor).map(|&row| starts[row].max(lo));
        let next_end = active.peek().map(|&Reverse((end, _))| end);
        let coordinate = match (next_start, next_end) {
            (Some(start), Some(end)) => start.min(end),
            (Some(start), None) => start,
            (None, Some(end)) => end,
            (None, None) => unreachable!(),
        };
        emit(&mut output, previous, coordinate, load, include_zero);
        while active
            .peek()
            .is_some_and(|&Reverse((end, _))| end == coordinate)
        {
            let Reverse((_, row)) = active.pop().unwrap();
            load = load
                .checked_sub(quantity(weights, row))
                .ok_or(IntervalError::LoadOverflow)?;
        }
        while cursor < rows.len() && starts[rows[cursor]].max(lo) == coordinate {
            let row = rows[cursor];
            load = load
                .checked_add(quantity(weights, row))
                .ok_or(IntervalError::LoadOverflow)?;
            active.push(Reverse((ends[row].min(hi), row)));
            cursor += 1;
        }
        previous = coordinate;
    }
    emit(&mut output, previous, hi, load, include_zero);
    Ok(output)
}

// Original production weighted preparation and iterator sweep, retained only
// as a private memory/locality comparison to the chosen flat-record route.
fn indices<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    i128: From<W>,
{
    let (domain, count) = validate(starts, ends, Some(weights), domain)?;
    let Some((lo, hi)) = domain else {
        return Ok(Vec::new());
    };
    let mut arrivals = Vec::with_capacity(count);
    for row in 0..starts.len() {
        let (start, end) = (starts[row].max(lo), ends[row].min(hi));
        if start < end && i128::from(weights[row]) > 0 {
            arrivals.push(row);
        }
    }
    let mut departures = arrivals.clone();
    if !arrivals.is_sorted_by_key(|&i| starts[i]) {
        arrivals.sort_unstable_by_key(|&i| starts[i]);
    }
    if !departures.is_sorted_by_key(|&i| ends[i]) {
        departures.sort_unstable_by_key(|&i| ends[i]);
    }
    index_sweep(
        arrivals
            .into_iter()
            .map(|i| (starts[i].max(lo), i128::from(weights[i]))),
        departures
            .into_iter()
            .map(|i| (ends[i].min(hi), i128::from(weights[i]))),
        (lo, hi),
        include_zero,
    )
}

fn index_sweep<T: Ord + Copy>(
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
        emit(&mut output, previous, coordinate, load, include_zero);
        if coordinate == right {
            return Ok(output);
        }
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

/// Independent O(n*u) membership oracle for SMALL, valid inputs only. It never
/// calls candidate validation, preprocessing, emission, or signed-event helpers.
pub fn oracle<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
    domain: Option<(T, T)>,
    include_zero: bool,
) -> Result<Vec<CoverageSegment<T>>, IntervalError>
where
    i128: From<W>,
{
    let domain = domain.or_else(|| {
        let lo = (0..starts.len())
            .filter(|&i| starts[i] < ends[i])
            .map(|i| starts[i])
            .min()?;
        let hi = (0..ends.len())
            .filter(|&i| starts[i] < ends[i])
            .map(|i| ends[i])
            .max()?;
        Some((lo, hi))
    });
    let Some((lo, hi)) = domain.filter(|(lo, hi)| lo < hi) else {
        return Ok(Vec::new());
    };
    let mut boundaries = vec![lo, hi];
    for row in 0..starts.len() {
        if starts[row] < ends[row] {
            for point in [starts[row], ends[row]] {
                if lo < point && point < hi {
                    boundaries.push(point);
                }
            }
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut cells = Vec::new();
    for pair in boundaries.windows(2) {
        let mut load = 0i128;
        for row in 0..starts.len() {
            if starts[row] <= pair[0] && pair[0] < ends[row] {
                load = load
                    .checked_add(weights.map_or(1, |w| i128::from(w[row])))
                    .ok_or(IntervalError::LoadOverflow)?;
            }
        }
        if include_zero || load > 0 {
            cells.push(CoverageSegment {
                start: pair[0],
                end: pair[1],
                load,
            });
        }
    }
    let mut canonical: Vec<CoverageSegment<T>> = Vec::new();
    for cell in cells {
        match canonical.last_mut() {
            Some(previous) if previous.end == cell.start && previous.load == cell.load => {
                previous.end = cell.end;
            }
            _ => canonical.push(cell),
        }
    }
    Ok(canonical)
}
