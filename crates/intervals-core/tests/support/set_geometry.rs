//! Independent oracles inspect original rows. They do not call merge or scans.

/// Classify each cell of the distinct input-boundary partition.
pub fn cells<T: Ord + Copy>(
    ls: &[T],
    le: &[T],
    rs: &[T],
    re: &[T],
    intersection: bool,
) -> Vec<(T, T)> {
    let mut boundaries: Vec<_> = ls.iter().chain(le).chain(rs).chain(re).copied().collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut output = Vec::new();
    let mut open = None;
    for pair in boundaries.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let left = ls.iter().zip(le).any(|(&s, &e)| s <= a && b <= e);
        let right = rs.iter().zip(re).any(|(&s, &e)| s <= a && b <= e);
        if left && right == intersection {
            open.get_or_insert(a);
        } else if let Some(start) = open.take() {
            output.push((start, a));
        }
    }
    if let Some(start) = open {
        output.push((start, *boundaries.last().unwrap()));
    }
    output
}

/// A second oracle for small integer inputs, using fixed-domain tick coverage.
#[allow(dead_code)]
pub fn bitmap(
    ls: &[i32],
    le: &[i32],
    rs: &[i32],
    re: &[i32],
    intersection: bool,
) -> Vec<(i32, i32)> {
    assert!(
        ls.iter()
            .chain(le)
            .chain(rs)
            .chain(re)
            .all(|&x| (-16..=16).contains(&x))
    );
    let mut left = [false; 32];
    let mut right = [false; 32];
    for (starts, ends, bits) in [(ls, le, &mut left), (rs, re, &mut right)] {
        for (&start, &end) in starts.iter().zip(ends) {
            for tick in start..end {
                bits[(tick + 16) as usize] = true;
            }
        }
    }
    let kept: Vec<_> = left
        .into_iter()
        .zip(right)
        .map(|(a, b)| a && b == intersection)
        .collect();
    let mut output = Vec::new();
    let mut index = 0;
    while index < kept.len() {
        if !kept[index] {
            index += 1;
            continue;
        }
        let start = index;
        while index < kept.len() && kept[index] {
            index += 1;
        }
        output.push((start as i32 - 16, index as i32 - 16));
    }
    output
}
