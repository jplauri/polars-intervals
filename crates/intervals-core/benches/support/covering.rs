//! Private oracles, shared by the release benchmark and the correctness tests.
#![allow(dead_code)]

/// Deliberately simple subset coverage oracle, independent of the greedy/DP.
pub fn covers<T: Ord + Copy>(starts: &[T], ends: &[T], mask: &[bool], left: T, right: T) -> bool {
    let mut selected: Vec<_> = (0..starts.len()).filter(|&i| mask[i]).collect();
    selected.sort_by_key(|&i| starts[i]);
    let mut frontier = left;
    for i in selected {
        if starts[i] > frontier {
            break;
        }
        frontier = frontier.max(ends[i]);
    }
    frontier >= right
}

pub fn objective(costs: &[i128], mask: &[bool]) -> (i128, usize) {
    mask.iter()
        .enumerate()
        .filter(|&(_, &selected)| selected)
        .fold((0, 0), |(cost, count), (i, _)| (cost + costs[i], count + 1))
}

pub fn brute_force(
    starts: &[i32],
    ends: &[i32],
    costs: &[i128],
    left: i32,
    right: i32,
) -> Option<(i128, usize)> {
    (0..1usize << starts.len())
        .filter_map(|bits| {
            let mask: Vec<_> = (0..starts.len()).map(|i| bits & (1 << i) != 0).collect();
            covers(starts, ends, &mask, left, right).then(|| objective(costs, &mask))
        })
        .min()
}
