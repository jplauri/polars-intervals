//! Private full-call offline sweep. Shares source preparation, not its oracle.
#[allow(dead_code)]
#[path = "../../src/coverage_stats.rs"]
mod preparation;

use intervals_core::{
    CoverageEndpoint, CoverageStats, CoverageStatsError, coverage_stats, validate_intervals,
};

pub const METHODS: &[&str] = &["production", "binary", "sweep"];

#[cfg(test)]
#[allow(dead_code)] // Also compiled by the harness-free benchmark's test target.
pub fn prefixes<T: CoverageEndpoint>(ss: &[T], se: &[T], boundaries: &[T]) -> Vec<i128> {
    let prepared = preparation::Prepared::new(ss, se);
    boundaries
        .iter()
        .map(|&x| {
            let k = prepared.union.partition_point(|&(_, end)| end <= x);
            prepared.prefix_at(x, k)
        })
        .collect()
}

pub fn run<T: CoverageEndpoint>(
    method: &str,
    qs: &[T],
    qe: &[T],
    ss: &[T],
    se: &[T],
) -> Result<Vec<CoverageStats>, CoverageStatsError> {
    if method == "production" {
        return coverage_stats(qs, qe, ss, se);
    }
    validate_intervals(qs, qe).map_err(CoverageStatsError::Queries)?;
    validate_intervals(ss, se).map_err(CoverageStatsError::Intervals)?;
    if qs.is_empty() {
        return Ok(Vec::new());
    }
    let prepared = preparation::Prepared::new(ss, se);
    match method {
        "binary" => preparation::direct(&prepared, qs, qe, false),
        "sweep" => preparation::sweep(&prepared, qs, qe),
        _ => panic!("unknown candidate {method}"),
    }
}
