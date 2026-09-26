//! Independent, test/benchmark-only oracles. Never linked into production.

/// Enumerate subsets of ALL integer coordinates in the relevant finite domain.
/// This intentionally knows nothing about predecessor-of-end candidates.
pub fn brute_stabbing(rows: &[(i64, i64)]) -> Option<usize> {
    if rows.iter().any(|&(s, e)| s >= e) {
        return None;
    }
    let Some(left) = rows.iter().map(|r| r.0).min() else {
        return Some(0);
    };
    let right = rows.iter().map(|r| r.1).max().unwrap();
    let width = (right - left) as u32;
    assert!(width <= 16, "oracle requires a small coordinate domain");
    (0u32..1 << width)
        .filter(|&mask| {
            rows.iter().all(|&(s, e)| {
                (left..right).any(|p| mask & (1 << (p - left)) != 0 && s <= p && p < e)
            })
        })
        .map(|mask| mask.count_ones() as usize)
        .min()
}

/// Enumerate interval subsets and check every pair, independently of stabbing.
pub fn brute_packing(rows: &[(i64, i64)]) -> usize {
    assert!(rows.len() <= 16);
    (0u32..1 << rows.len())
        .filter(|&mask| {
            (0..rows.len()).all(|i| {
                (0..i).all(|j| {
                    mask & (1 << i) == 0
                        || mask & (1 << j) == 0
                        || rows[i].1 <= rows[j].0
                        || rows[j].1 <= rows[i].0
                })
            })
        })
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap()
}

/// Large-instance structural oracle: right-to-left maximum interval packing.
/// Orders STARTS descending, never computes stabbing points or predecessors.
pub fn packing<T: Ord + Copy>(starts: &[T], ends: &[T]) -> usize {
    let mut rows: Vec<_> = starts.iter().copied().zip(ends.iter().copied()).collect();
    rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0));
    let mut boundary = None;
    let mut count = 0;
    for (s, e) in rows {
        if boundary.is_none_or(|b| e <= b) {
            boundary = Some(s);
            count += 1;
        }
    }
    count
}
