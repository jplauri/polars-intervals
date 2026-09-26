use super::capacity::{self, Network};
use super::reference;
use std::time::Instant;

pub struct Measurement {
    pub mask: Vec<bool>,
    pub preparation_ns: u128,
    pub flow_ns: u128,
    pub reconstruction_ns: u128,
}

pub fn run<const PROFILE: bool>(
    name: &str,
    s: &[i64],
    e: &[i64],
    w: &[i64],
    k: usize,
) -> Measurement {
    let mut result = Measurement {
        mask: Vec::new(),
        preparation_ns: 0,
        flow_ns: 0,
        reconstruction_ns: 0,
    };
    if name == "production" {
        result.mask = capacity::max_weight_with_capacity(s, e, w, k).unwrap();
        return result;
    }
    if name == "generic" {
        result.mask = reference::solve(s, e, w, k);
        return result;
    }
    if name == "capacity_one" {
        result.mask = intervals_core::max_weight_non_overlapping(s, e, w).unwrap();
        return result;
    }
    let begin = Timer::new::<PROFILE>();
    let prepared = capacity::prepare(s, e, w, k).unwrap();
    let rows = prepared.rows;
    result.mask = prepared.mask;
    if name != "whole" && capacity::concurrency(&rows) <= k {
        for row in &rows {
            result.mask[row.index] = true;
        }
        result.preparation_ns = begin.elapsed();
        return result;
    }
    let ranges = if name == "whole" || name == "whole_guarded" {
        std::iter::once(0..rows.len()).collect()
    } else {
        capacity::components(&rows)
    };
    result.preparation_ns = begin.elapsed();
    if name == "parallel" && ranges.len() >= 8 {
        let begin = Timer::new::<PROFILE>();
        capacity::solve_parallel(&rows, &ranges, k, &mut result.mask, 8).unwrap();
        result.flow_ns = begin.elapsed();
    } else {
        for range in ranges {
            let component = &rows[range];
            if component.is_empty() {
                continue;
            }
            let begin = Timer::new::<PROFILE>();
            if name != "whole" && capacity::concurrency(component) <= k {
                result.preparation_ns += begin.elapsed();
                let begin = Timer::new::<PROFILE>();
                for row in component {
                    result.mask[row.index] = true;
                }
                result.reconstruction_ns += begin.elapsed();
            } else {
                let mut network = Network::new(component, k);
                result.preparation_ns += begin.elapsed();
                let begin = Timer::new::<PROFILE>();
                network.solve(k).unwrap();
                result.flow_ns += begin.elapsed();
                let begin = Timer::new::<PROFILE>();
                network.reconstruct(component, &mut result.mask);
                result.reconstruction_ns += begin.elapsed();
            }
        }
    }
    capacity::check_objective(w, &result.mask).unwrap();
    result
}

struct Timer(Option<Instant>);
impl Timer {
    fn new<const PROFILE: bool>() -> Self {
        Self(if PROFILE { Some(Instant::now()) } else { None })
    }
    fn elapsed(&self) -> u128 {
        self.0.map_or(0, |start| start.elapsed().as_nanos())
    }
}
