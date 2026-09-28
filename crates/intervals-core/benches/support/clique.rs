//! Private complete-call clique candidates, shared by benchmarks and tests only.
use intervals_core::IntervalError;
use std::{cmp::Reverse, collections::BinaryHeap};

pub const METHODS: &[&str] = &[
    "events",
    "events_index",
    "streams",
    "streams_sort",
    "indices",
    "heap",
    "common",
    "quadratic",
];

fn add(a: i128, b: i128) -> Result<i128, IntervalError> {
    a.checked_add(b).ok_or(IntervalError::WeightOverflow)
}

fn value<W: Copy>(weights: Option<&[W]>, i: usize) -> i128
where
    i128: From<W>,
{
    weights.map_or(1, |w| i128::from(w[i]))
}

// Only the winning coordinate or isolated vertex is retained. No active-set
// snapshots: even nested inputs with an improvement on every arrival stay linear.
#[derive(Clone, Copy)]
struct Best<T> {
    weight: i128,
    point: Option<T>,
    empty: Option<usize>,
}

impl<T: Ord + Copy> Best<T> {
    fn observe(&mut self, point: T, weight: i128) {
        if weight > 0
            && (weight > self.weight
                || (weight == self.weight && self.point.is_none_or(|old| point < old)))
        {
            self.weight = weight;
            self.point = Some(point);
            self.empty = None;
        }
    }
}

/// Each candidate validates all rows before taking any shortcut. Prevalidation
/// also counts useful rows so allocations reflect useful input, not discarded rows.
fn validate<T: Ord + Copy, W: Copy>(
    starts: &[T],
    ends: &[T],
    weights: Option<&[W]>,
) -> Result<(Best<T>, usize), IntervalError>
where
    i128: From<W>,
{
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    if let Some(w) = weights
        && w.len() != starts.len()
    {
        return Err(IntervalError::WeightLengthMismatch {
            intervals_len: starts.len(),
            weights_len: w.len(),
        });
    }
    let mut best = Best {
        weight: 0,
        point: None,
        empty: None,
    };
    let mut useful = 0;
    for i in 0..starts.len() {
        if starts[i] > ends[i] {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        let w = value(weights, i);
        if w > 0 {
            if starts[i] == ends[i] {
                if w > best.weight {
                    best.weight = w;
                    best.empty = Some(i);
                }
            } else {
                useful += 1;
            }
        }
    }
    Ok((best, useful))
}

fn indices<T: Ord + Copy, W: Copy>(s: &[T], e: &[T], w: Option<&[W]>, m: usize) -> Vec<usize>
where
    i128: From<W>,
{
    let mut rows = Vec::with_capacity(m);
    rows.extend((0..s.len()).filter(|&i| s[i] < e[i] && value(w, i) > 0));
    rows
}

/// A signed delta permits a compact event with no separately padded kind field.
/// i128 payload alignment still makes (i64, i128) 32 bytes on the measured target.
fn events<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: Option<&[W]>,
    m: usize,
    best: &mut Best<T>,
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    if w.is_none() {
        // Unit events need only a coordinate and departure/arrival discriminator.
        let mut events = Vec::with_capacity(2 * m);
        for i in 0..s.len() {
            if s[i] < e[i] {
                events.extend([(s[i], true), (e[i], false)]);
            }
        }
        events.sort_unstable();
        let mut active = 0i128;
        let mut i = 0;
        while i < events.len() {
            let point = events[i].0;
            while i < events.len() && events[i].0 == point {
                active = add(active, if events[i].1 { 1 } else { -1 })?;
                i += 1;
            }
            best.observe(point, active);
        }
    } else {
        let mut events = Vec::with_capacity(2 * m);
        for i in 0..s.len() {
            let weight = value(w, i);
            if s[i] < e[i] && weight > 0 {
                events.extend([(s[i], weight), (e[i], -weight)]);
            }
        }
        // Negative deltas precede positive deltas at equal coordinates.
        events.sort_unstable();
        let mut active = 0;
        let mut i = 0;
        while i < events.len() {
            let point = events[i].0;
            while i < events.len() && events[i].0 == point {
                active = add(active, events[i].1)?;
                i += 1;
            }
            best.observe(point, active);
        }
    }
    Ok(())
}

fn index_events<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: &[W],
    m: usize,
    best: &mut Best<T>,
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let mut events = Vec::with_capacity(2 * m);
    for i in 0..s.len() {
        if s[i] < e[i] && i128::from(w[i]) > 0 {
            // The low bit marks starts; the remaining bits carry the row.
            // A nonempty interval requires nonzero-sized T, so its allocated
            // endpoint slices are bounded by isize::MAX bytes.
            events.extend([(s[i], (i << 1) | 1), (e[i], i << 1)]);
        }
    }
    events.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| (a.1 & 1).cmp(&(b.1 & 1))));
    let (mut i, mut active) = (0, 0);
    while i < events.len() {
        let point = events[i].0;
        while i < events.len() && events[i].0 == point {
            let tag = events[i].1;
            let weight = i128::from(w[tag >> 1]);
            active = add(active, if tag & 1 == 1 { weight } else { -weight })?;
            i += 1;
        }
        best.observe(point, active);
    }
    Ok(())
}

fn endpoint_streams<T: Ord + Copy>(
    s: &[T],
    e: &[T],
    m: usize,
    best: &mut Best<T>,
    detect: bool,
) -> Result<(), IntervalError> {
    let mut starts = Vec::with_capacity(m);
    let mut ends = Vec::with_capacity(m);
    for i in 0..s.len() {
        if s[i] < e[i] {
            starts.push(s[i]);
            ends.push(e[i]);
        }
    }
    // Check both relevant streams independently; ordered starts imply nothing
    // about the order of ends. Include detection in the complete timed call.
    if !detect || !starts.is_sorted() {
        starts.sort_unstable();
    }
    if !detect || !ends.is_sorted() {
        ends.sort_unstable();
    }
    let (mut a, mut b, mut active) = (0, 0, 0i128);
    while a < m {
        let point = starts[a];
        while b < m && ends[b] <= point {
            active = add(active, -1)?;
            b += 1;
        }
        while a < m && starts[a] == point {
            active = add(active, 1)?;
            a += 1;
        }
        best.observe(point, active);
    }
    Ok(())
}

fn streams<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: &[W],
    m: usize,
    best: &mut Best<T>,
    detect: bool,
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    // Native W avoids inflating ordinary 64-bit weights into padded i128 records.
    let mut starts = Vec::with_capacity(m);
    let mut ends = Vec::with_capacity(m);
    for i in 0..s.len() {
        if s[i] < e[i] && i128::from(w[i]) > 0 {
            starts.push((s[i], w[i]));
            ends.push((e[i], w[i]));
        }
    }
    if !detect || !starts.is_sorted_by_key(|x| x.0) {
        starts.sort_unstable_by_key(|x| x.0);
    }
    if !detect || !ends.is_sorted_by_key(|x| x.0) {
        ends.sort_unstable_by_key(|x| x.0);
    }
    let (mut a, mut b, mut active) = (0, 0, 0i128);
    while a < m {
        let point = starts[a].0;
        while b < m && ends[b].0 <= point {
            active = add(active, -i128::from(ends[b].1))?;
            b += 1;
        }
        while a < m && starts[a].0 == point {
            active = add(active, i128::from(starts[a].1))?;
            a += 1;
        }
        best.observe(point, active);
    }
    Ok(())
}

fn index_streams<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: &[W],
    m: usize,
    best: &mut Best<T>,
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let mut starts = indices(s, e, Some(w), m);
    let mut ends = starts.clone();
    if !starts.is_sorted_by_key(|&i| s[i]) {
        starts.sort_unstable_by_key(|&i| s[i]);
    }
    if !ends.is_sorted_by_key(|&i| e[i]) {
        ends.sort_unstable_by_key(|&i| e[i]);
    }
    let (mut a, mut b, mut active) = (0, 0, 0i128);
    while a < m {
        let point = s[starts[a]];
        while b < m && e[ends[b]] <= point {
            active = add(active, -i128::from(w[ends[b]]))?;
            b += 1;
        }
        while a < m && s[starts[a]] == point {
            active = add(active, i128::from(w[starts[a]]))?;
            a += 1;
        }
        best.observe(point, active);
    }
    Ok(())
}

fn heap<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: Option<&[W]>,
    m: usize,
    best: &mut Best<T>,
) -> Result<(), IntervalError>
where
    i128: From<W>,
{
    let mut starts = indices(s, e, w, m);
    if !starts.is_sorted_by_key(|&i| s[i]) {
        starts.sort_unstable_by_key(|&i| s[i]);
    }
    // Deliberately grows with actual concurrency, never reserve m heap entries.
    let mut ends = BinaryHeap::<Reverse<(T, usize)>>::new();
    let (mut a, mut active) = (0, 0i128);
    while a < m {
        let point = s[starts[a]];
        while let Some(&Reverse((end, row))) = ends.peek() {
            if end > point {
                break;
            }
            active = add(active, -value(w, row))?;
            ends.pop();
        }
        while a < m && s[starts[a]] == point {
            let row = starts[a];
            active = add(active, value(w, row))?;
            ends.push(Reverse((e[row], row)));
            a += 1;
        }
        best.observe(point, active);
    }
    Ok(())
}

pub fn run<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: Option<&[W]>,
    method: &str,
) -> Result<Vec<bool>, IntervalError>
where
    i128: From<W>,
{
    let (mut best, m) = validate(s, e, w)?;
    if m > 0 {
        match method {
            "events" => events(s, e, w, m, &mut best)?,
            "events_index" => {
                if let Some(w) = w {
                    index_events(s, e, w, m, &mut best)?;
                } else {
                    events(s, e, w, m, &mut best)?;
                }
            }
            "streams" | "streams_sort" | "indices" if w.is_none() => {
                endpoint_streams(s, e, m, &mut best, method != "streams_sort")?
            }
            "streams" | "streams_sort" => {
                streams(s, e, w.unwrap(), m, &mut best, method != "streams_sort")?
            }
            "indices" => index_streams(s, e, w.unwrap(), m, &mut best)?,
            "heap" => heap(s, e, w, m, &mut best)?,
            "common" => {
                let mut lo = None;
                let mut hi = None;
                for i in 0..s.len() {
                    if s[i] < e[i] && value(w, i) > 0 {
                        lo = Some(lo.map_or(s[i], |old: T| old.max(s[i])));
                        hi = Some(hi.map_or(e[i], |old: T| old.min(e[i])));
                    }
                }
                if lo.unwrap() < hi.unwrap() {
                    // Sum only after establishing one feasible clique. Summing
                    // unrelated positives here would produce spurious overflow.
                    let mut total = 0;
                    for i in 0..s.len() {
                        if s[i] < e[i] && value(w, i) > 0 {
                            total = add(total, value(w, i))?;
                        }
                    }
                    best.observe(lo.unwrap(), total);
                } else if let Some(w) = w {
                    streams(s, e, w, m, &mut best, true)?;
                } else {
                    endpoint_streams(s, e, m, &mut best, true)?;
                }
            }
            "quadratic" => {
                // Allocation-light secondary reference. Candidate points stay in
                // original order, so observe explicitly breaks coordinate ties.
                for i in 0..s.len() {
                    if s[i] < e[i] && value(w, i) > 0 {
                        let mut score = 0;
                        for j in 0..s.len() {
                            let weight = value(w, j);
                            if weight > 0 && s[j] <= s[i] && s[i] < e[j] {
                                score = add(score, weight)?;
                            }
                        }
                        best.observe(s[i], score);
                    }
                }
            }
            _ => panic!("unknown clique candidate: {method}"),
        }
    }
    // The sole output pass restores original row order and excludes empties at
    // any selected coordinate. No n-sized weight vector is needed for units.
    Ok((0..s.len())
        .map(|i| {
            best.point.map_or(best.empty == Some(i), |point| {
                value(w, i) > 0 && s[i] <= point && point < e[i]
            })
        })
        .collect())
}
