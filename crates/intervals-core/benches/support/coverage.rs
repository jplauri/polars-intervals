//! Experimental kernels. Shared by release benchmarks and proptest, never linked
//! into the library. The brute-force oracle is in tests/support/coverage.rs.
use std::cmp::Reverse;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(pub i128, pub Reverse<usize>);

impl Score {
    fn add(self, length: i128) -> Self {
        Self(self.0 + length, Reverse(self.1.0 + 1))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub start: i64,
    pub end: i64,
    pub row: usize,
}

pub trait Records {
    fn len(&self) -> usize;
    fn get(&self, i: usize) -> Record;
}

struct Prefix<'a, R>(&'a R, usize);
impl<R: Records> Records for Prefix<'_, R> {
    fn len(&self) -> usize {
        self.1
    }
    fn get(&self, i: usize) -> Record {
        self.0.get(i)
    }
}

impl Records for Vec<Record> {
    fn len(&self) -> usize {
        self.len()
    }
    fn get(&self, i: usize) -> Record {
        self[i]
    }
}

struct Indirect<'a> {
    order: Vec<usize>,
    starts: &'a [i64],
    ends: &'a [i64],
}
impl Records for Indirect<'_> {
    fn len(&self) -> usize {
        self.order.len()
    }
    fn get(&self, i: usize) -> Record {
        let row = self.order[i];
        Record {
            start: self.starts[row],
            end: self.ends[row],
            row,
        }
    }
}

pub fn packed(starts: &[i64], ends: &[i64], prune: bool) -> Vec<Record> {
    let mut rows: Vec<_> = starts
        .iter()
        .zip(ends)
        .enumerate()
        .filter(|(_, (s, e))| s < e)
        .map(|(row, (&start, &end))| Record { start, end, row })
        .collect();
    rows.sort_unstable_by_key(|r| (r.end, Reverse(r.start), Reverse(r.row)));
    if prune {
        let mut left = None;
        rows.reverse();
        rows.retain(|r| {
            if left.is_none_or(|l| r.start < l) {
                left = Some(r.start);
                true
            } else {
                false
            }
        });
        rows.reverse();
    }
    rows
}

fn indirect<'a>(starts: &'a [i64], ends: &'a [i64]) -> Indirect<'a> {
    let mut order: Vec<_> = (0..starts.len()).filter(|&i| starts[i] < ends[i]).collect();
    order.sort_unstable_by_key(|&i| (ends[i], Reverse(starts[i]), Reverse(i)));
    let mut left = None;
    order.reverse();
    order.retain(|&i| {
        if left.is_none_or(|l| starts[i] < l) {
            left = Some(starts[i]);
            true
        } else {
            false
        }
    });
    order.reverse();
    Indirect {
        order,
        starts,
        ends,
    }
}

/// Prefix-length phi; psi is a one-based forced-last state, zero = missing.
pub fn helpers(rows: &impl Records, sweep: bool, pruned: bool) -> (Vec<usize>, Vec<usize>) {
    let mut phi = vec![0; rows.len()];
    let mut psi = phi.clone();
    let mut p = 0;
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..rows.len() {
        let r = rows.get(i);
        if sweep {
            assert!(pruned);
            while p < i && rows.get(p).end <= r.start {
                p += 1;
            }
        } else {
            let (mut lo, mut hi) = (0, i);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if rows.get(mid).end <= r.start {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            p = lo;
        }
        phi[i] = p;
        if pruned {
            if p < i {
                psi[i] = p + 1;
            }
        } else {
            // Prefix skyline: a popped interval cannot be the leftmost-start
            // overlapping predecessor in a later query; its container suffices.
            let pos = stack.partition_point(|&j| rows.get(j).end <= r.start);
            if pos < stack.len() && rows.get(stack[pos]).start < r.start {
                psi[i] = stack[pos] + 1;
            }
            while stack.last().is_some_and(|&j| rows.get(j).start >= r.start) {
                stack.pop();
            }
            stack.push(i);
        }
    }
    (phi, psi)
}

#[derive(Default)]
pub struct Run {
    pub mask: Vec<bool>,
    pub score: Score,
    pub sort: Duration,
    pub preprocess: Duration,
    pub dp: Duration,
    pub reconstruct: Duration,
}

#[allow(clippy::too_many_arguments)]
fn layer(
    rows: &impl Records,
    phi: &[usize],
    psi: &[usize],
    budget: usize,
    previous: &[Score],
    forced_previous: &[Score],
    current: &mut [Score],
    forced: &mut [Score],
    decisions: &mut [u8],
) {
    for i in 0..rows.len() {
        let r = rows.get(i);
        let mut take = previous[phi[i]].add(i128::from(r.end) - i128::from(r.start));
        let mut decision = 0;
        if budget > 1 && psi[i] > 0 {
            let other = forced_previous[psi[i]]
                .add(i128::from(r.end) - i128::from(rows.get(psi[i] - 1).end));
            if other > take {
                take = other;
                decision = 2;
            }
        }
        forced[i + 1] = take;
        current[i + 1] = current[i].max(take);
        if take > current[i] {
            decision |= 1;
        }
        decisions[i] = decision;
    }
}

fn trace(
    rows: &impl Records,
    phi: &[usize],
    psi: &[usize],
    decisions: &[Vec<u8>],
    mask: &mut [bool],
    k: usize,
) {
    let (mut i, mut j, mut forced) = (rows.len(), k, false);
    while i > 0 && j > 0 {
        let d = decisions[j - 1][i - 1];
        if !forced && d & 1 == 0 {
            i -= 1;
            continue;
        }
        mask[rows.get(i - 1).row] = true;
        forced = d & 2 != 0;
        i = if forced { psi[i - 1] } else { phi[i - 1] };
        j -= 1;
    }
}

fn kernel(
    rows: &impl Records,
    phi: &[usize],
    psi: &[usize],
    k: usize,
    n: usize,
    mode: &str,
) -> Run {
    let k = k.min(rows.len());
    let width = rows.len() + 1;
    let mut result = Run {
        mask: vec![false; n],
        ..Run::default()
    };
    let begin = Instant::now();
    if mode == "full" || mode == "unpruned" {
        let mut prefix = vec![vec![Score::default(); width]; k + 1];
        let mut forced = prefix.clone();
        let mut decisions = vec![vec![0; width - 1]; k];
        for j in 1..=k {
            let (p, c) = prefix.split_at_mut(j);
            let (fp, fc) = forced.split_at_mut(j);
            layer(
                rows,
                phi,
                psi,
                j,
                &p[j - 1],
                &fp[j - 1],
                &mut c[0],
                &mut fc[0],
                &mut decisions[j - 1],
            );
        }
        result.score = prefix[k][width - 1];
        result.dp = begin.elapsed();
        let begin = Instant::now();
        trace(rows, phi, psi, &decisions, &mut result.mask, k);
        result.reconstruct = begin.elapsed();
    } else {
        let mut previous = vec![Score::default(); width];
        let mut forced_previous = previous.clone();
        let mut current = previous.clone();
        let mut forced = previous.clone();
        let mut last = vec![0; width - 1];
        let mut decisions = Vec::new();
        for j in 1..=k {
            layer(
                rows,
                phi,
                psi,
                j,
                &previous,
                &forced_previous,
                &mut current,
                &mut forced,
                &mut last,
            );
            if mode != "recompute" {
                decisions.push(last.clone());
            }
            std::mem::swap(&mut previous, &mut current);
            std::mem::swap(&mut forced_previous, &mut forced);
        }
        result.score = previous[width - 1];
        result.dp = begin.elapsed();
        let begin = Instant::now();
        if mode == "recompute" {
            let (mut i, mut j, mut must_take) = (width - 1, k, false);
            while i > 0 && j > 0 {
                while i > 0 && !must_take && last[i - 1] & 1 == 0 {
                    i -= 1;
                }
                if i == 0 {
                    break;
                }
                result.mask[rows.get(i - 1).row] = true;
                must_take = last[i - 1] & 2 != 0;
                i = if must_take { psi[i - 1] } else { phi[i - 1] };
                j -= 1;
                if i == 0 || j == 0 {
                    break;
                }
                previous.fill(Score::default());
                forced_previous.fill(Score::default());
                for budget in 1..=j {
                    layer(
                        &Prefix(rows, i),
                        phi,
                        psi,
                        budget,
                        &previous,
                        &forced_previous,
                        &mut current,
                        &mut forced,
                        &mut last,
                    );
                    std::mem::swap(&mut previous, &mut current);
                    std::mem::swap(&mut forced_previous, &mut forced);
                }
            }
        } else {
            trace(rows, phi, psi, &decisions, &mut result.mask, k);
        }
        result.reconstruct = begin.elapsed();
    }
    result
}

pub const METHODS: &[&str] = &[
    "full",
    "rolling",
    "recompute",
    "binary",
    "indirect",
    "unpruned",
    "components",
    "production",
];

pub fn run(starts: &[i64], ends: &[i64], k: usize, mode: &str) -> Run {
    if mode == "production" {
        return Run {
            mask: intervals_core::max_k_coverage(starts, ends, k).unwrap(),
            ..Run::default()
        };
    }
    let begin = Instant::now();
    if mode == "indirect" {
        let rows = indirect(starts, ends);
        let sort = begin.elapsed();
        let begin = Instant::now();
        let (phi, psi) = helpers(&rows, true, true);
        let preprocess = begin.elapsed();
        let mut result = kernel(&rows, &phi, &psi, k, starts.len(), mode);
        result.sort = sort;
        result.preprocess = preprocess;
        return result;
    }
    let rows = packed(starts, ends, mode != "unpruned");
    let sort = begin.elapsed();
    if mode == "components" {
        let mut result = components(&rows, k, starts.len());
        result.sort = sort;
        return result;
    }
    let begin = Instant::now();
    let (phi, psi) = helpers(
        &rows,
        mode != "binary" && mode != "unpruned",
        mode != "unpruned",
    );
    let preprocess = begin.elapsed();
    let mut result = kernel(&rows, &phi, &psi, k, starts.len(), mode);
    result.sort = sort;
    result.preprocess = preprocess;
    result
}

fn components(rows: &[Record], k: usize, n: usize) -> Run {
    let k = k.min(rows.len());
    let begin = Instant::now();
    let mut ranges = Vec::new();
    let mut left = 0;
    for i in 1..=rows.len() {
        if i == rows.len() || rows[i - 1].end < rows[i].start {
            ranges.push(left..i);
            left = i;
        }
    }
    let mut global = vec![Score::default(); k + 1];
    let mut splits = Vec::new();
    for range in &ranges {
        let part = rows[range.clone()].to_vec();
        let (phi, psi) = helpers(&part, true, true);
        let limit = k.min(part.len());
        let mut curve = vec![Score::default(); limit + 1];
        let mut previous = vec![Score::default(); part.len() + 1];
        let mut fp = previous.clone();
        let mut current = previous.clone();
        let mut fc = previous.clone();
        let mut decisions = vec![0; part.len()];
        for (j, score) in curve.iter_mut().enumerate().skip(1) {
            layer(
                &part,
                &phi,
                &psi,
                j,
                &previous,
                &fp,
                &mut current,
                &mut fc,
                &mut decisions,
            );
            *score = current[part.len()];
            std::mem::swap(&mut previous, &mut current);
            std::mem::swap(&mut fp, &mut fc);
        }
        let mut next = global.clone();
        let mut split = vec![0; k + 1];
        for b in 0..=k {
            for j in 1..=b.min(limit) {
                let candidate = Score(
                    global[b - j].0 + curve[j].0,
                    Reverse(global[b - j].1.0 + curve[j].1.0),
                );
                if candidate > next[b] {
                    next[b] = candidate;
                    split[b] = j;
                }
            }
        }
        global = next;
        splits.push(split);
    }
    let mut result = Run {
        score: global[k],
        mask: vec![false; n],
        dp: begin.elapsed(),
        ..Run::default()
    };
    let begin = Instant::now();
    let mut budget = k;
    for (range, split) in ranges.iter().zip(&splits).rev() {
        let j = split[budget];
        if j > 0 {
            let mut part = rows[range.clone()].to_vec();
            for (i, r) in part.iter_mut().enumerate() {
                r.row = i;
            }
            let (phi, psi) = helpers(&part, true, true);
            let selection = kernel(&part, &phi, &psi, j, part.len(), "rolling");
            for (r, selected) in rows[range.clone()].iter().zip(selection.mask) {
                result.mask[r.row] = selected;
            }
        }
        budget -= j;
    }
    result.reconstruct = begin.elapsed();
    result
}
