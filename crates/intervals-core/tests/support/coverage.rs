//! Deliberately independent exhaustive oracle: no DP, predecessor or pruning code.
pub fn objective<T: Copy + Ord + Into<i128>>(
    starts: &[T],
    ends: &[T],
    mask: &[bool],
) -> (i128, usize) {
    assert_eq!(starts.len(), mask.len());
    let mut intervals: Vec<_> = starts
        .iter()
        .zip(ends)
        .zip(mask)
        .filter(|(_, selected)| **selected)
        .map(|((&s, &e), _)| (s.into(), e.into()))
        .collect();
    let count = intervals.len();
    intervals.sort_unstable();
    let mut total = 0;
    let mut segment = None;
    for (start, end) in intervals {
        match segment {
            Some((left, right)) if start <= right => segment = Some((left, right.max(end))),
            Some((left, right)) => {
                total += right - left;
                segment = Some((start, end));
            }
            None => segment = Some((start, end)),
        }
    }
    if let Some((left, right)) = segment {
        total += right - left;
    }
    (total, count)
}

pub fn brute(starts: &[i64], ends: &[i64], k: usize) -> (i128, usize) {
    let mut best = (0, 0);
    for bits in 0usize..1 << starts.len() {
        if bits.count_ones() as usize > k {
            continue;
        }
        let mask: Vec<_> = (0..starts.len()).map(|i| bits & (1 << i) != 0).collect();
        let candidate = objective(starts, ends, &mask);
        if candidate.0 > best.0 || (candidate.0 == best.0 && candidate.1 < best.1) {
            best = candidate;
        }
    }
    best
}

/// Independent O(k n²) reference. Enumerate *every* possible preceding selected
/// interval rather than using phi/psi, dominance pruning, or the published
/// two-family optimization. A minimum-count optimum contains no contained pair,
/// so its starts and ends strictly increase. With that invariant, the last
/// selected interval alone determines the incremental union length.
pub fn quadratic(starts: &[i64], ends: &[i64], k: usize) -> (i128, usize) {
    use std::cmp::Reverse;
    let mut rows: Vec<_> = starts
        .iter()
        .copied()
        .zip(ends.iter().copied())
        .filter(|(s, e)| s < e)
        .collect();
    rows.sort_unstable_by_key(|&(s, e)| (e, s));
    let mut previous = vec![(0i128, Reverse(0usize)); rows.len()];
    let mut answer = (0i128, Reverse(0usize));
    for budget in 1..=k.min(rows.len()) {
        let mut current = previous.clone();
        for (i, &(start, end)) in rows.iter().enumerate() {
            current[i] = (i128::from(end) - i128::from(start), Reverse(1));
            if budget > 1 {
                for (j, &(left, right)) in rows[..i].iter().enumerate() {
                    if left < start && right < end {
                        let extra = i128::from(end) - i128::from(start.max(right));
                        current[i] =
                            current[i].max((previous[j].0 + extra, Reverse(previous[j].1.0 + 1)));
                    }
                }
            }
            answer = answer.max(current[i]);
        }
        previous = current;
    }
    (answer.0, answer.1.0)
}
