//! Original-row/cell oracle, independent of source merging or prefix searches.
use intervals_core::{CoverageEndpoint, CoverageStats};

pub fn cells<T: CoverageEndpoint>(qs: &[T], qe: &[T], ss: &[T], se: &[T]) -> Vec<CoverageStats> {
    qs.iter()
        .zip(qe)
        .map(|(&a, &b)| {
            let query_length = b.widened() - a.widened();
            let overlap_count = ss
                .iter()
                .zip(se)
                .filter(|&(&s, &e)| a < b && s < e && s < b && a < e)
                .count() as u64;
            let mut boundaries = vec![a, b];
            for (&s, &e) in ss.iter().zip(se) {
                if s < e && s < b && a < e {
                    boundaries.extend([a.max(s), b.min(e)]);
                }
            }
            boundaries.sort_unstable();
            boundaries.dedup();
            let covered_length = boundaries
                .windows(2)
                .filter(|pair| {
                    ss.iter()
                        .zip(se)
                        .any(|(&s, &e)| s <= pair[0] && pair[1] <= e)
                })
                .map(|pair| pair[1].widened() - pair[0].widened())
                .sum();
            CoverageStats {
                overlap_count,
                covered_length,
                query_length,
                covered_fraction: (a < b).then(|| covered_length as f64 / query_length as f64),
            }
        })
        .collect()
}

#[allow(dead_code)]
pub fn bitmap(qs: &[i64], qe: &[i64], ss: &[i64], se: &[i64]) -> Vec<i128> {
    qs.iter()
        .zip(qe)
        .map(|(&a, &b)| {
            (a..b)
                .filter(|&tick| ss.iter().zip(se).any(|(&s, &e)| s <= tick && tick < e))
                .count() as i128
        })
        .collect()
}
