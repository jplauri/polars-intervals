//! Independent lane oracles, shared by the benchmarks and correctness tests.
#[path = "random.rs"]
mod random;
pub use random::shuffle;

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
