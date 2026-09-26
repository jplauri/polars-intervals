//! Private exact implementation candidates, instrumented only for experiments.
use intervals_core::{DiscreteEndpoint, IntervalError};
use std::time::Instant;

pub struct Run<T> {
    pub points: Vec<T>,
    pub validation_ns: u128,
    pub preprocessing_ns: u128,
    pub sort_ns: u128,
    pub scan_ns: u128,
}

pub fn run<T: DiscreteEndpoint>(
    starts: &[T],
    ends: &[T],
    method: &str,
) -> Result<Run<T>, IntervalError> {
    let begin = Instant::now();
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    for (index, (&s, &e)) in starts.iter().zip(ends).enumerate() {
        if s > e {
            return Err(IntervalError::InvalidInterval { index });
        }
        if s == e {
            return Err(IntervalError::EmptyInterval { index });
        }
    }
    let validation_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut result = Run {
        points: Vec::new(),
        validation_ns,
        preprocessing_ns: 0,
        sort_ns: 0,
        scan_ns: 0,
    };
    if method == "C" && ends.is_sorted() {
        result.preprocessing_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        result.points = scan(starts.iter().copied().zip(ends.iter().copied()));
        result.scan_ns = begin.elapsed().as_nanos();
    } else if method == "B" {
        let mut order: Vec<_> = (0..starts.len()).collect();
        result.preprocessing_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        order.sort_unstable_by_key(|&i| ends[i]);
        result.sort_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        result.points = scan(order.into_iter().map(|i| (starts[i], ends[i])));
        result.scan_ns = begin.elapsed().as_nanos();
    } else {
        assert!(method == "A" || method == "C");
        let mut rows: Vec<_> = starts.iter().copied().zip(ends.iter().copied()).collect();
        result.preprocessing_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        rows.sort_unstable_by_key(|&(_, e)| e);
        result.sort_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        result.points = scan(rows);
        result.scan_ns = begin.elapsed().as_nanos();
    }
    Ok(result)
}

fn scan<T: DiscreteEndpoint>(rows: impl IntoIterator<Item = (T, T)>) -> Vec<T> {
    let mut points = Vec::new();
    for (s, e) in rows {
        if points.last().is_none_or(|&p| p < s) {
            points.push(e.predecessor().unwrap());
        }
    }
    points
}
