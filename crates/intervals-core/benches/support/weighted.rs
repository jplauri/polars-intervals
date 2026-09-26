//! Private exact reference candidates. Shared only by benchmarks and tests.
use intervals_core::IntervalError;
use std::mem::size_of;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub struct Measurement {
    pub mask: Vec<bool>,
    pub preprocessing_ns: u128,
    pub optimization_ns: u128,
    pub reconstruction_ns: u128,
    // Peak simultaneously live Vec capacities, excluding inputs/allocator overhead.
    pub peak_buffer_bytes: usize,
    pub buffer_allocations: usize,
}

pub type Candidate = fn(&[i64], &[i64], &[i64]) -> Result<Measurement, IntervalError>;
pub const CANDIDATES: [(&str, Candidate); 3] = [
    ("A_binary", binary),
    ("B_two_orders", two_orders),
    ("C_events", events),
];

struct Prepared {
    rows: Vec<usize>,
    mask: Vec<bool>,
    empty_weight: i128,
}

fn prepare(s: &[i64], e: &[i64], w: &[i64]) -> Result<Prepared, IntervalError> {
    if s.len() != e.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: s.len(),
            ends_len: e.len(),
        });
    }
    if s.len() != w.len() {
        return Err(IntervalError::WeightLengthMismatch {
            intervals_len: s.len(),
            weights_len: w.len(),
        });
    }
    let mut result = Prepared {
        rows: Vec::with_capacity(s.len()),
        mask: vec![false; s.len()],
        empty_weight: 0,
    };
    for i in 0..s.len() {
        if s[i] > e[i] {
            return Err(IntervalError::InvalidInterval { index: i });
        }
        if w[i] > 0 {
            if s[i] == e[i] {
                result.mask[i] = true;
                result.empty_weight = add(result.empty_weight, i128::from(w[i]))?;
            } else {
                result.rows.push(i);
            }
        }
    }
    Ok(result)
}

fn add(a: i128, b: i128) -> Result<i128, IntervalError> {
    a.checked_add(b).ok_or(IntervalError::WeightOverflow)
}

fn bytes<T>(v: &Vec<T>) -> usize {
    v.capacity() * size_of::<T>()
}

pub fn binary(s: &[i64], e: &[i64], w: &[i64]) -> Result<Measurement, IntervalError> {
    ordered::<false>(s, e, w)
}

pub fn two_orders(s: &[i64], e: &[i64], w: &[i64]) -> Result<Measurement, IntervalError> {
    ordered::<true>(s, e, w)
}

fn ordered<const SWEEP: bool>(
    s: &[i64],
    e: &[i64],
    w: &[i64],
) -> Result<Measurement, IntervalError> {
    let begin = Instant::now();
    let Prepared {
        mut rows,
        mut mask,
        empty_weight,
    } = prepare(s, e, w)?;
    rows.sort_unstable_by_key(|&i| (e[i], s[i], i));
    let ends: Vec<_> = rows.iter().map(|&i| e[i]).collect();
    let mut predecessors = Vec::with_capacity(rows.len());
    let base_bytes = bytes(&rows) + bytes(&mask) + bytes(&ends) + bytes(&predecessors);
    let mut peak_bytes = base_bytes;
    if SWEEP {
        // Indices into finish order, independently sorted by start.
        let mut by_start: Vec<_> = (0..rows.len()).collect();
        by_start.sort_unstable_by_key(|&j| (s[rows[j]], e[rows[j]], rows[j]));
        predecessors.resize(rows.len(), 0);
        let mut p = 0;
        for &j in &by_start {
            while p < ends.len() && ends[p] <= s[rows[j]] {
                p += 1;
            }
            predecessors[j] = p;
        }
        peak_bytes = peak_bytes.max(base_bytes + bytes(&by_start));
    }
    let preprocessing_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut optimum = vec![0i128; rows.len() + 1];
    peak_bytes = peak_bytes.max(base_bytes + bytes(&optimum));
    for (j, &i) in rows.iter().enumerate() {
        if !SWEEP {
            predecessors.push(ends[..j].partition_point(|&end| end <= s[i]));
        }
        optimum[j + 1] = optimum[j].max(add(optimum[predecessors[j]], i128::from(w[i]))?);
    }
    add(empty_weight, optimum[rows.len()])?;
    let optimization_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut j = rows.len();
    while j > 0 {
        if optimum[j] > optimum[j - 1] {
            mask[rows[j - 1]] = true;
            j = predecessors[j - 1];
        } else {
            j -= 1;
        }
    }
    Ok(Measurement {
        mask,
        preprocessing_ns,
        optimization_ns,
        reconstruction_ns: begin.elapsed().as_nanos(),
        peak_buffer_bytes: peak_bytes,
        buffer_allocations: 1
            + 2 * usize::from(!s.is_empty())
            + (2 + usize::from(SWEEP)) * usize::from(!rows.is_empty()),
    })
}

pub fn events(s: &[i64], e: &[i64], w: &[i64]) -> Result<Measurement, IntervalError> {
    let begin = Instant::now();
    let Prepared {
        rows,
        mut mask,
        empty_weight,
    } = prepare(s, e, w)?;
    let mut events = Vec::with_capacity(2 * rows.len());
    for (j, &i) in rows.iter().enumerate() {
        // false (end) sorts before true (start) at a touching coordinate.
        events.push((s[i], true, j));
        events.push((e[i], false, j));
    }
    events.sort_unstable();
    let preprocessing_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut at_start = vec![0i128; rows.len()];
    let mut previous = vec![usize::MAX; rows.len()];
    let (mut best, mut head) = (0i128, usize::MAX);
    for &(_, is_start, j) in &events {
        if is_start {
            at_start[j] = best;
            previous[j] = head;
        } else {
            let include = add(at_start[j], i128::from(w[rows[j]]))?;
            if include > best {
                best = include;
                head = j;
            }
        }
    }
    add(empty_weight, best)?;
    let optimization_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    while head != usize::MAX {
        mask[rows[head]] = true;
        head = previous[head];
    }
    let reconstruction_ns = begin.elapsed().as_nanos();
    let peak_buffer_bytes =
        bytes(&rows) + bytes(&mask) + bytes(&events) + bytes(&at_start) + bytes(&previous);
    Ok(Measurement {
        mask,
        preprocessing_ns,
        optimization_ns,
        reconstruction_ns,
        peak_buffer_bytes,
        buffer_allocations: 2 * usize::from(!s.is_empty()) + 3 * usize::from(!rows.is_empty()),
    })
}

/// Independent feasibility check: sort only the selected non-empty intervals.
pub fn verify(s: &[i64], e: &[i64], w: &[i64], mask: &[bool]) -> i128 {
    assert_eq!(s.len(), mask.len());
    let mut chosen = Vec::new();
    let mut objective = 0i128;
    for i in 0..s.len() {
        if mask[i] {
            assert!(w[i] > 0);
            objective = objective.checked_add(i128::from(w[i])).unwrap();
            if s[i] < e[i] {
                chosen.push((s[i], e[i]));
            }
        }
        if s[i] == e[i] && w[i] > 0 {
            assert!(mask[i]);
        }
    }
    chosen.sort_unstable();
    assert!(chosen.windows(2).all(|pair| pair[0].1 <= pair[1].0));
    objective
}

/// Independent start-ordered suffix recurrence for large benchmark inputs.
/// No shared preparation, finish ordering, predecessor links or reconstruction.
pub fn suffix_optimum(s: &[i64], e: &[i64], w: &[i64]) -> i128 {
    let mut rows = Vec::new();
    let mut empty = 0i128;
    for i in 0..s.len() {
        if s[i] == e[i] {
            empty += i128::from(w[i].max(0));
        } else {
            rows.push((s[i], e[i], w[i]));
        }
    }
    rows.sort_unstable();
    let mut suffix = vec![0i128; rows.len() + 1];
    for i in (0..rows.len()).rev() {
        let next = rows.partition_point(|row| row.0 < rows[i].1);
        suffix[i] = suffix[i + 1].max(i128::from(rows[i].2) + suffix[next]);
    }
    empty + suffix[0]
}

pub fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

pub fn shuffle<T>(values: &mut [T], state: &mut u64) {
    for i in (1..values.len()).rev() {
        values.swap(i, (random(state) % (i + 1) as u64) as usize);
    }
}
