//! Independent original-problem oracles. No production preparation or frontier.
use std::collections::BTreeSet;

pub fn graph<T: Ord + Copy>(starts: &[T], ends: &[T], touching: bool) -> Vec<u32> {
    let mut ids = vec![None; starts.len()];
    let mut next = 0;
    for root in 0..starts.len() {
        if ids[root].is_some() {
            continue;
        }
        let mut pending = vec![root];
        ids[root] = Some(next);
        while let Some(a) = pending.pop() {
            for b in 0..starts.len() {
                let relation = starts[a] < ends[a]
                    && starts[b] < ends[b]
                    && if touching {
                        starts[a] <= ends[b] && starts[b] <= ends[a]
                    } else {
                        starts[a] < ends[b] && starts[b] < ends[a]
                    };
                if relation && ids[b].is_none() {
                    ids[b] = Some(next);
                    pending.push(b);
                }
            }
        }
        next += 1;
    }
    ids.into_iter().map(Option::unwrap).collect()
}

pub fn cells<T: Ord + Copy>(starts: &[T], ends: &[T], domain: Option<(T, T)>) -> Vec<(T, T)> {
    let mut boundaries: BTreeSet<T> = starts.iter().chain(ends).copied().collect();
    if let Some((l, r)) = domain {
        boundaries.extend([l, r]);
    }
    let boundaries: Vec<_> = boundaries.into_iter().collect();
    let mut out: Vec<(T, T)> = Vec::new();
    for cell in boundaries.windows(2) {
        let (a, b) = (cell[0], cell[1]);
        let covered = starts.iter().zip(ends).any(|(&s, &e)| s <= a && a < e);
        let selected = domain.map_or(covered, |(l, r)| l <= a && b <= r && !covered);
        if selected {
            if let Some(last) = out.last_mut()
                && last.1 == a
            {
                last.1 = b;
            } else {
                out.push((a, b));
            }
        }
    }
    out
}

pub fn bitmap(starts: &[i32], ends: &[i32], domain: Option<(i32, i32)>) -> Vec<(i32, i32)> {
    let (l, r) = domain.unwrap_or_else(|| {
        (
            *starts.iter().min().unwrap_or(&0),
            *ends.iter().max().unwrap_or(&0),
        )
    });
    let mut out = Vec::new();
    let mut open = None;
    for tick in l..r {
        let covered = starts
            .iter()
            .zip(ends)
            .any(|(&s, &e)| s <= tick && tick < e);
        if covered == domain.is_none() {
            open.get_or_insert(tick);
        } else if let Some(start) = open.take() {
            out.push((start, tick));
        }
    }
    if let Some(start) = open {
        out.push((start, r));
    }
    out
}
