//! Private comparison implementations, shared with the correctness tests.
use intervals_core::IntervalError;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::mem::size_of;

pub type Coloring = Result<(Vec<u32>, usize), IntervalError>;
pub type Candidate = fn(&[i64], &[i64]) -> Coloring;
pub const CANDIDATES: [(&str, Candidate); 3] =
    [("heap", heap), ("two_sorts", two_sorts), ("events", events)];

fn nonempty(starts: &[i64], ends: &[i64]) -> Result<Vec<usize>, IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    let mut indices = Vec::with_capacity(starts.len());
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        if start < end {
            indices.push(i);
        }
    }
    Ok(indices)
}

// The second return value is peak live Vec/heap capacity in bytes, including
// output, excluding input, allocator overhead and transient reallocations.
pub fn heap(starts: &[i64], ends: &[i64]) -> Coloring {
    let mut order = nonempty(starts, ends)?;
    order.sort_unstable_by_key(|&i| (starts[i], i));
    let mut lanes = vec![0; starts.len()];
    let mut active = BinaryHeap::<Reverse<(i64, u32)>>::new();
    for i in order.iter().copied() {
        if let Some(mut earliest) = active.peek_mut()
            && earliest.0.0 <= starts[i]
        {
            lanes[i] = earliest.0.1;
            *earliest = Reverse((ends[i], lanes[i]));
            continue;
        }
        let lane = u32::try_from(active.len()).map_err(|_| IntervalError::TooManyLanes)?;
        lanes[i] = lane;
        active.push(Reverse((ends[i], lane)));
    }
    let bytes = order.capacity() * size_of::<usize>()
        + lanes.capacity() * size_of::<u32>()
        + active.capacity() * size_of::<(i64, u32)>();
    Ok((lanes, bytes))
}

pub fn two_sorts(starts: &[i64], ends: &[i64]) -> Coloring {
    let mut start_order = nonempty(starts, ends)?;
    let mut end_order = start_order.clone();
    start_order.sort_unstable_by_key(|&i| (starts[i], i));
    end_order.sort_unstable_by_key(|&i| (ends[i], i));
    let mut lanes = vec![0; starts.len()];
    let mut free = Vec::new();
    let (mut ended, mut next) = (0, 0usize);
    for i in start_order.iter().copied() {
        // Every released non-empty interval started strictly before this start,
        // so its lane is already assigned. Empty intervals never enter either list.
        while ended < end_order.len() && ends[end_order[ended]] <= starts[i] {
            free.push(lanes[end_order[ended]]);
            ended += 1;
        }
        lanes[i] = match free.pop() {
            Some(lane) => lane,
            None => {
                let lane = u32::try_from(next).map_err(|_| IntervalError::TooManyLanes)?;
                next += 1;
                lane
            }
        };
    }
    let bytes = (start_order.capacity() + end_order.capacity()) * size_of::<usize>()
        + (lanes.capacity() + free.capacity()) * size_of::<u32>();
    Ok((lanes, bytes))
}

pub fn events(starts: &[i64], ends: &[i64]) -> Coloring {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    let mut events = Vec::with_capacity(2 * starts.len());
    for (i, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        if start < end {
            events.push((start, true, i));
            events.push((end, false, i)); // false sorts first: END before START.
        }
    }
    events.sort_unstable();
    let mut lanes = vec![0; starts.len()];
    let mut free = Vec::new();
    let mut next = 0usize;
    for &(_, is_start, i) in &events {
        if !is_start {
            free.push(lanes[i]);
        } else {
            lanes[i] = match free.pop() {
                Some(lane) => lane,
                None => {
                    let lane = u32::try_from(next).map_err(|_| IntervalError::TooManyLanes)?;
                    next += 1;
                    lane
                }
            };
        }
    }
    let bytes = events.capacity() * size_of::<(i64, bool, usize)>()
        + (lanes.capacity() + free.capacity()) * size_of::<u32>();
    Ok((lanes, bytes))
}

/// Independent optimality oracle: signed endpoint deltas, END before START.
pub fn optimum(starts: &[i64], ends: &[i64]) -> usize {
    let mut events = Vec::new();
    for (&start, &end) in starts.iter().zip(ends) {
        if start < end {
            events.extend([(start, 1i64), (end, -1)]);
        }
    }
    events.sort_unstable();
    let (mut active, mut peak) = (0, 0);
    for (_, delta) in events {
        active += delta;
        peak = peak.max(active);
    }
    (peak as usize).max(usize::from(!starts.is_empty()))
}

/// Scalable validity oracle: within each lane, adjacent non-empty intervals
/// sorted by start must not overlap. Also checks output length and contiguous IDs.
pub fn verify(starts: &[i64], ends: &[i64], lanes: &[u32], expected: usize) {
    assert_eq!(lanes.len(), starts.len());
    let mut ids = lanes.to_vec();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), expected);
    assert!(ids.iter().enumerate().all(|(i, &lane)| i == lane as usize));
    let mut order: Vec<_> = (0..starts.len()).filter(|&i| starts[i] < ends[i]).collect();
    order.sort_unstable_by_key(|&i| (lanes[i], starts[i], i));
    for pair in order.windows(2) {
        let (i, j) = (pair[0], pair[1]);
        assert!(lanes[i] != lanes[j] || ends[i] <= starts[j]);
    }
}

// Fixed PRNG for reproducible inputs and method ordering, independent of rand versions.
pub fn shuffle<T>(values: &mut [T], state: &mut u64) {
    for i in (1..values.len()).rev() {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        values.swap(i, (*state % (i as u64 + 1)) as usize);
    }
}
