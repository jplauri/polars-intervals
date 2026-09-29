//! Private exact oracles. Shared only by benchmarks and tests.

/// Independent feasibility check: sort only the selected non-empty intervals.
pub fn verify(s: &[i64], e: &[i64], w: &[i64], mask: &[bool]) -> i128 {
    assert_eq!(s.len(), mask.len());
    let mut chosen = Vec::new();
    let mut objective = 0i128;
    for i in 0..s.len() {
        if mask[i] {
            assert!(w[i] > 0);
            objective = objective.checked_add(i128::from(w[i])).unwrap();
            if s[i] < e[i] {
                chosen.push((s[i], e[i]));
            }
        }
        if s[i] == e[i] && w[i] > 0 {
            assert!(mask[i]);
        }
    }
    chosen.sort_unstable();
    assert!(chosen.windows(2).all(|pair| pair[0].1 <= pair[1].0));
    objective
}

/// Independent start-ordered suffix recurrence for large benchmark inputs.
/// No shared preparation, finish ordering, predecessor links or reconstruction.
pub fn suffix_optimum(s: &[i64], e: &[i64], w: &[i64]) -> i128 {
    let mut rows = Vec::new();
    let mut empty = 0i128;
    for i in 0..s.len() {
        if s[i] == e[i] {
            empty += i128::from(w[i].max(0));
        } else {
            rows.push((s[i], e[i], w[i]));
        }
    }
    rows.sort_unstable();
    let mut suffix = vec![0i128; rows.len() + 1];
    for i in (0..rows.len()).rev() {
        let next = rows.partition_point(|row| row.0 < rows[i].1);
        suffix[i] = suffix[i + 1].max(i128::from(rows[i].2) + suffix[next]);
    }
    empty + suffix[0]
}
