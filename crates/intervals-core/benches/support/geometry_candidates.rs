//! Private complete-call comparisons with and without sortedness checks.
//! Indexed preparation avoids endpoint copies, but still allocates indices and output.
use intervals_core::{
    IntervalError, cluster_intervals, interval_gaps, merge_intervals, validate_intervals,
};

pub const CLUSTER_METHODS: &[&str] = &[
    "production",
    "packed_sort",
    "packed_sorted",
    "indices_sorted",
];
pub const MERGE_METHODS: &[&str] = CLUSTER_METHODS;
pub const GAP_METHODS: &[&str] = &[
    "production",
    "packed_sort",
    "packed_sorted",
    "indices_sorted",
    "materialized",
];

pub fn cluster<T: Ord + Copy>(
    method: &str,
    starts: &[T],
    ends: &[T],
    touching: bool,
) -> Result<Vec<u32>, IntervalError> {
    if method == "production" {
        return cluster_intervals(starts, ends, touching);
    }
    validate_intervals(starts, ends)?;
    if method == "packed_sort" || method == "packed_sorted" {
        let mut rows: Vec<_> = starts
            .iter()
            .zip(ends)
            .enumerate()
            .filter_map(|(i, (&s, &e))| (s < e).then_some((s, e, i)))
            .collect();
        if method == "packed_sort" || !rows.is_sorted_by_key(|r| r.0) {
            rows.sort_unstable_by_key(|r| r.0);
        }
        label(starts, ends, rows.into_iter(), touching)
    } else {
        let order = indices(starts, ends, None);
        label(
            starts,
            ends,
            order.into_iter().map(|i| (starts[i], ends[i], i)),
            touching,
        )
    }
}

fn label<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    rows: impl Iterator<Item = (T, T, usize)>,
    touching: bool,
) -> Result<Vec<u32>, IntervalError> {
    let mut labels = vec![0; starts.len()];
    let mut remap = Vec::<Option<u32>>::new();
    for (i, (&s, &e)) in starts.iter().zip(ends).enumerate() {
        if s == e {
            labels[i] = u32::try_from(remap.len()).map_err(|_| IntervalError::TooManyClusters)?;
            remap.push(None);
        }
    }
    let mut frontier = None;
    let mut current = 0;
    for (s, e, i) in rows {
        if frontier.is_none_or(|r| if touching { s > r } else { s >= r }) {
            current = u32::try_from(remap.len()).map_err(|_| IntervalError::TooManyClusters)?;
            remap.push(None);
            frontier = Some(e);
        } else {
            frontier = Some(frontier.unwrap().max(e));
        }
        labels[i] = current;
    }
    let mut next = 0u64;
    for id in &mut labels {
        let slot = &mut remap[*id as usize];
        *id = match *slot {
            Some(id) => id,
            None => {
                let id = u32::try_from(next).map_err(|_| IntervalError::TooManyClusters)?;
                next += 1;
                *slot = Some(id);
                id
            }
        };
    }
    Ok(labels)
}

pub fn merge<T: Ord + Copy>(
    method: &str,
    starts: &[T],
    ends: &[T],
) -> Result<Vec<(T, T)>, IntervalError> {
    if method == "production" {
        return merge_intervals(starts, ends);
    }
    validate_intervals(starts, ends)?;
    Ok(if method == "packed_sort" || method == "packed_sorted" {
        coalesce(packed(starts, ends, None, method == "packed_sorted").into_iter())
    } else {
        coalesce(
            indices(starts, ends, None)
                .into_iter()
                .map(|i| (starts[i], ends[i])),
        )
    })
}

pub fn gaps<T: Ord + Copy>(
    method: &str,
    starts: &[T],
    ends: &[T],
    left: T,
    right: T,
) -> Result<Vec<(T, T)>, IntervalError> {
    if method == "production" {
        return interval_gaps(starts, ends, left, right);
    }
    validate_intervals(starts, ends)?;
    if left > right {
        return Err(IntervalError::InvalidDomain);
    }
    if left == right {
        return Ok(Vec::new());
    }
    let domain = Some((left, right));
    Ok(match method {
        "packed_sort" => complement(packed(starts, ends, domain, false).into_iter(), left, right),
        "packed_sorted" => complement(packed(starts, ends, domain, true).into_iter(), left, right),
        "materialized" => {
            let union = coalesce(packed(starts, ends, domain, true).into_iter());
            complement(union.into_iter(), left, right)
        }
        _ => complement(
            indices(starts, ends, domain)
                .into_iter()
                .map(|i| (starts[i].max(left), ends[i].min(right))),
            left,
            right,
        ),
    })
}

fn packed<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    domain: Option<(T, T)>,
    sorted: bool,
) -> Vec<(T, T)> {
    let mut rows: Vec<_> = starts
        .iter()
        .zip(ends)
        .filter_map(|(&s, &e)| {
            let (s, e) = domain.map_or((s, e), |(l, r)| (s.max(l), e.min(r)));
            (s < e).then_some((s, e))
        })
        .collect();
    if !sorted || !rows.is_sorted_by_key(|r| r.0) {
        rows.sort_unstable_by_key(|r| r.0);
    }
    rows
}

fn indices<T: Ord + Copy>(starts: &[T], ends: &[T], domain: Option<(T, T)>) -> Vec<usize> {
    let mut rows: Vec<_> = (0..starts.len())
        .filter(|&i| {
            starts[i] < ends[i] && domain.is_none_or(|(l, r)| starts[i] < r && ends[i] > l)
        })
        .collect();
    if !rows.is_sorted_by_key(|&i| starts[i]) {
        rows.sort_unstable_by_key(|&i| starts[i]);
    }
    rows
}

fn coalesce<T: Ord + Copy>(rows: impl Iterator<Item = (T, T)>) -> Vec<(T, T)> {
    let mut out: Vec<(T, T)> = Vec::new();
    for (s, e) in rows {
        if let Some(last) = out.last_mut()
            && s <= last.1
        {
            last.1 = last.1.max(e);
        } else {
            out.push((s, e));
        }
    }
    out
}

fn complement<T: Ord + Copy>(rows: impl Iterator<Item = (T, T)>, left: T, right: T) -> Vec<(T, T)> {
    let mut out = Vec::new();
    let mut cursor = left;
    for (s, e) in rows {
        if cursor < s {
            out.push((cursor, s));
        }
        cursor = cursor.max(e);
    }
    if cursor < right {
        out.push((cursor, right));
    }
    out
}
