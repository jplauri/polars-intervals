//! Private release-benchmark candidates, also compiled by the oracle tests.
#![allow(dead_code)]
use intervals_core::IntervalError;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

#[path = "../../src/cover.rs"]
pub mod production;
use production::{Best, better};

pub struct Measurement {
    pub result: Result<Vec<bool>, IntervalError>,
    pub preprocessing_ns: u128,
    pub optimization_ns: u128,
    pub reconstruction_ns: u128,
}

pub fn cardinality<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    left: T,
    right: T,
    method: &str,
) -> Measurement {
    let begin = Instant::now();
    let mut candidates = Vec::new();
    let mut order = Vec::new();
    if method == "MC-B" {
        // Only original indices are kept/sorted in this candidate.
        assert_eq!(starts.len(), ends.len());
        assert!(left <= right);
        for i in 0..starts.len() {
            assert!(starts[i] <= ends[i]);
            if starts[i].max(left) < ends[i].min(right) {
                order.push(i);
            }
        }
        order.sort_unstable_by_key(|&i| (starts[i].max(left), i));
    } else {
        candidates = production::prepare(starts, ends, left, right).unwrap();
        if method != "MC-A-detect" || !candidates.windows(2).all(|w| w[0].start <= w[1].start) {
            candidates.sort_unstable_by_key(|c| (c.start, c.row));
        }
    }
    let preprocessing_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut mask = vec![false; starts.len()];
    let result = match method {
        "MC-A" | "MC-A-detect" => production::greedy(&candidates, left, right, &mut mask),
        "MC-B" => {
            let (mut frontier, mut next) = (left, 0);
            while frontier < right {
                let mut best = None;
                while next < order.len() && starts[order[next]].max(left) <= frontier {
                    let row = order[next];
                    let end = ends[row].min(right);
                    if end > frontier
                        && best.is_none_or(|(e, i)| (end, Reverse(row)) > (e, Reverse(i)))
                    {
                        best = Some((end, row));
                    }
                    next += 1;
                }
                let Some((end, row)) = best else { break };
                mask[row] = true;
                frontier = end;
            }
            if frontier == right {
                Ok(())
            } else {
                Err(IntervalError::InfeasibleCover)
            }
        }
        "MC-C" => {
            let (mut frontier, mut next) = (left, 0);
            let mut heap = BinaryHeap::new();
            while frontier < right {
                while next < candidates.len() && candidates[next].start <= frontier {
                    let c = candidates[next];
                    heap.push((c.end, Reverse(c.row)));
                    next += 1;
                }
                let Some((end, Reverse(row))) = heap.pop() else {
                    break;
                };
                if end <= frontier {
                    break;
                }
                mask[row] = true;
                frontier = end;
            }
            if frontier == right {
                Ok(())
            } else {
                Err(IntervalError::InfeasibleCover)
            }
        }
        _ => panic!("unknown candidate"),
    }
    .map(|()| mask);
    Measurement {
        result,
        preprocessing_ns,
        optimization_ns: begin.elapsed().as_nanos(),
        reconstruction_ns: 0,
    }
}

pub struct SegmentTree {
    tree: Vec<Best>,
    size: usize,
}
impl SegmentTree {
    pub fn new(len: usize) -> Self {
        let size = len.next_power_of_two();
        Self {
            tree: vec![None; size * 2],
            size,
        }
    }
    pub fn update(&mut self, coordinate: usize, value: Best) {
        let mut i = self.size + coordinate;
        self.tree[i] = better(self.tree[i], value);
        while i > 1 {
            i /= 2;
            let best = better(self.tree[2 * i], self.tree[2 * i + 1]);
            if best == self.tree[i] {
                break;
            }
            self.tree[i] = best;
        }
    }
    pub fn query(&self, left: usize, right: usize) -> Best {
        let (mut l, mut r, mut best) = (self.size + left, self.size + right, None);
        while l < r {
            if l % 2 == 1 {
                best = better(best, self.tree[l]);
                l += 1;
            }
            if r % 2 == 1 {
                r -= 1;
                best = better(best, self.tree[r]);
            }
            l /= 2;
            r /= 2;
        }
        best
    }
}

pub fn weighted<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    costs: &[i128],
    left: T,
    right: T,
    method: &str,
) -> Measurement {
    let begin = Instant::now();
    production::validate_costs(costs, starts.len()).unwrap();
    let mut candidates = production::prepare(starts, ends, left, right).unwrap();
    candidates.sort_unstable_by_key(|c| (c.end, c.row));
    let coordinates = production::coordinates(&candidates, left);
    let preprocessing_ns = begin.elapsed().as_nanos();
    if left == right || coordinates.last() != Some(&right) {
        return Measurement {
            result: if left == right {
                Ok(vec![false; starts.len()])
            } else {
                Err(IntervalError::InfeasibleCover)
            },
            preprocessing_ns,
            optimization_ns: 0,
            reconstruction_ns: 0,
        };
    }
    let begin = Instant::now();
    let solution = if method == "MCC-A" {
        production::dynamic_program(&candidates, &coordinates, costs)
    } else {
        let mut tree = SegmentTree::new(if method == "MCC-B" {
            coordinates.len()
        } else {
            1
        });
        let mut dp = if method == "MCC-C" {
            vec![None; coordinates.len()]
        } else {
            Vec::new()
        };
        let mut back = vec![None; coordinates.len()];
        let initial = Some((0i128, 0usize, 0usize));
        if method == "MCC-C" {
            dp[0] = initial;
        }
        tree.update(0, initial);
        let mut next = 0;
        for r in 1..coordinates.len() {
            let mut best = None;
            while next < candidates.len() && candidates[next].end == coordinates[r] {
                let c = candidates[next];
                let l = coordinates.partition_point(|&x| x < c.start);
                let predecessor = match method {
                    "MCC-B" => tree.query(l, r),
                    "MCC-C" => dp[l..r].iter().copied().fold(None, better),
                    _ => panic!("unknown candidate"),
                };
                if let Some((cost, count, predecessor)) = predecessor {
                    let proposal = (
                        cost.checked_add(costs[c.row]).unwrap(),
                        count + 1,
                        c.row,
                        predecessor,
                    );
                    best = Some(
                        best.map_or(proposal, |b: (i128, usize, usize, usize)| b.min(proposal)),
                    );
                }
                next += 1;
            }
            if let Some((cost, count, row, predecessor)) = best {
                back[r] = Some((predecessor, row));
                if method == "MCC-B" {
                    tree.update(r, Some((cost, count, r)));
                } else {
                    dp[r] = Some((cost, count, r));
                }
            }
        }
        production::Solution {
            back,
            overflowed: false,
        }
    };
    let optimization_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut mask = vec![false; starts.len()];
    let result = if coordinates.last() == Some(&right)
        && production::reconstruct(&solution.back, &mut mask)
    {
        Ok(mask)
    } else {
        Err(IntervalError::InfeasibleCover)
    };
    Measurement {
        result,
        preprocessing_ns,
        optimization_ns,
        reconstruction_ns: begin.elapsed().as_nanos(),
    }
}

/// Deliberately simple subset coverage oracle, independent of candidates/DP.
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
