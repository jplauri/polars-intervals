//! Private complete-call set-geometry comparisons. All validate before preparing.
use intervals_core::{
    IntervalSetError, intersect_intervals, subtract_intervals, validate_intervals,
};

pub const METHODS: &[&str] = &["union_scan", "fused", "events"];

pub fn subtract<T: Ord + Copy>(
    method: &str,
    ls: &[T],
    le: &[T],
    rs: &[T],
    re: &[T],
) -> Result<Vec<(T, T)>, IntervalSetError> {
    if method == "union_scan" {
        return subtract_intervals(ls, le, rs, re);
    }
    validate_intervals(ls, le).map_err(IntervalSetError::Left)?;
    validate_intervals(rs, re).map_err(IntervalSetError::Right)?;
    match method {
        "fused" => Ok(fused_subtract(runs(ls, le), runs(rs, re))),
        "events" => Ok(events(ls, le, rs, re, false)),
        _ => panic!("unknown candidate: {method}"),
    }
}

pub fn intersect<T: Ord + Copy>(
    method: &str,
    ls: &[T],
    le: &[T],
    rs: &[T],
    re: &[T],
) -> Result<Vec<(T, T)>, IntervalSetError> {
    if method == "union_scan" {
        return intersect_intervals(ls, le, rs, re);
    }
    validate_intervals(ls, le).map_err(IntervalSetError::Left)?;
    validate_intervals(rs, re).map_err(IntervalSetError::Right)?;
    match method {
        "fused" => Ok(fused_intersect(runs(ls, le), runs(rs, re))),
        "events" => Ok(events(ls, le, rs, re, true)),
        _ => panic!("unknown candidate: {method}"),
    }
}

// Packed preparation retains O(n + m) endpoint pairs. Fusion removes the two
// materialized union vectors, not this preparation or the output allocation.
fn runs<T: Ord + Copy>(starts: &[T], ends: &[T]) -> impl Iterator<Item = (T, T)> {
    let mut rows: Vec<_> = starts
        .iter()
        .zip(ends)
        .filter_map(|(&s, &e)| (s < e).then_some((s, e)))
        .collect();
    if !rows.is_sorted_by_key(|r| r.0) {
        rows.sort_unstable_by_key(|r| r.0);
    }
    let mut rows = rows.into_iter().peekable();
    std::iter::from_fn(move || {
        let (start, mut end) = rows.next()?;
        while let Some(&(next_start, next_end)) = rows.peek()
            && next_start <= end
        {
            end = end.max(next_end);
            rows.next();
        }
        Some((start, end))
    })
}

fn fused_subtract<T: Ord + Copy>(
    left: impl Iterator<Item = (T, T)>,
    right: impl Iterator<Item = (T, T)>,
) -> Vec<(T, T)> {
    let mut right = right.peekable();
    let mut output = Vec::new();
    for (mut cursor, end) in left {
        while right.peek().is_some_and(|r| r.1 <= cursor) {
            right.next();
        }
        while let Some(&(start, right_end)) = right.peek()
            && start < end
        {
            if cursor < start {
                output.push((cursor, start));
            }
            cursor = cursor.max(right_end);
            if cursor >= end {
                break;
            }
            right.next();
        }
        if cursor < end {
            output.push((cursor, end));
        }
    }
    output
}

fn fused_intersect<T: Ord + Copy>(
    mut left: impl Iterator<Item = (T, T)>,
    mut right: impl Iterator<Item = (T, T)>,
) -> Vec<(T, T)> {
    let mut a = left.next();
    let mut b = right.next();
    let mut output = Vec::new();
    while let (Some((ls, le)), Some((rs, re))) = (a, b) {
        let start = ls.max(rs);
        let end = le.min(re);
        if start < end {
            output.push((start, end));
        }
        if le <= re {
            a = left.next();
        }
        if re <= le {
            b = right.next();
        }
    }
    output
}

fn events<T: Ord + Copy>(
    ls: &[T],
    le: &[T],
    rs: &[T],
    re: &[T],
    intersection: bool,
) -> Vec<(T, T)> {
    let mut events = Vec::new();
    for (side, (starts, ends)) in [(ls, le), (rs, re)].into_iter().enumerate() {
        for (&start, &end) in starts.iter().zip(ends) {
            if start < end {
                events.push((start, side, true));
                events.push((end, side, false));
            }
        }
    }
    // Ends precede starts at a tie. Process the entire coordinate batch before
    // inspecting membership, so counts never create spurious boundaries.
    events.sort_unstable();
    let mut active = [0usize; 2];
    let mut output: Vec<(T, T)> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let coordinate = events[i].0;
        while i < events.len() && events[i].0 == coordinate {
            let (_, side, start) = events[i];
            active[side] = if start {
                active[side].checked_add(1).expect("count bounded by rows")
            } else {
                active[side].checked_sub(1).expect("end follows start")
            };
            i += 1;
        }
        if i < events.len() && active[0] > 0 && (active[1] > 0) == intersection {
            let end = events[i].0;
            if let Some(last) = output.last_mut()
                && last.1 == coordinate
            {
                last.1 = end;
            } else {
                output.push((coordinate, end));
            }
        }
    }
    output
}
