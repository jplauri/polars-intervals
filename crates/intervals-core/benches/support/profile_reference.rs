//! Benchmark-only exact oracles. Not linked into the production library.
pub type Job = (i64, i64, i64);
pub type Segment = (i64, i64, usize);

fn normalize(profile: &[Segment], limit: usize) -> Vec<Segment> {
    let mut rows = profile.to_vec();
    rows.sort_unstable_by_key(|r| (r.0, r.1));
    let mut result: Vec<Segment> = Vec::new();
    for (s, e, c) in rows {
        assert!(s <= e);
        if s == e {
            continue;
        }
        let c = c.min(limit);
        if let Some(&last) = result.last() {
            assert!(last.1 <= s);
            if last.1 < s {
                push(&mut result, (last.1, s, 0));
            }
        }
        push(&mut result, (s, e, c));
    }
    result
}

fn push(result: &mut Vec<Segment>, row: Segment) {
    if let Some(last) = result.last_mut()
        && last.1 == row.0
        && last.2 == row.2
    {
        last.1 = row.1;
    } else {
        result.push(row);
    }
}

fn capacity_at(profile: &[Segment], t: i64) -> usize {
    let i = profile.partition_point(|r| r.1 <= t);
    profile.get(i).filter(|r| r.0 <= t).map_or(0, |r| r.2)
}

fn feasible(jobs: &[Job], rows: &[usize], profile: &[Segment]) -> bool {
    let mut events: Vec<_> = rows
        .iter()
        .flat_map(|&i| [(jobs[i].0, 1i64), (jobs[i].1, -1)])
        .collect();
    events.extend(profile.iter().flat_map(|r| [(r.0, 0), (r.1, 0)]));
    events.sort_unstable();
    let mut active = 0i64;
    let mut i = 0;
    while i < events.len() {
        let t = events[i].0;
        while i < events.len() && events[i].0 == t {
            active += events[i].1;
            i += 1;
        }
        if active as usize > capacity_at(profile, t) {
            return false;
        }
    }
    true
}

pub fn verify(jobs: &[Job], profile: &[Segment], mask: &[bool]) -> i128 {
    assert_eq!(jobs.len(), mask.len());
    let rows: Vec<_> = (0..jobs.len())
        .filter(|&i| mask[i] && jobs[i].0 < jobs[i].1)
        .collect();
    assert!(
        feasible(jobs, &rows, &normalize(profile, jobs.len())),
        "infeasible mask"
    );
    jobs.iter()
        .zip(mask)
        .map(|(&(s, e, w), &selected)| {
            if s == e && w > 0 {
                assert!(selected);
            }
            if selected {
                assert!(w > 0);
                i128::from(w)
            } else {
                0
            }
        })
        .sum()
}

/// Exhaustive independent atomic-segment oracle; no graph or sweep reuse.
pub fn brute_force(jobs: &[Job], profile: &[Segment]) -> i128 {
    assert!(jobs.len() <= 12);
    let mut points: Vec<_> = jobs
        .iter()
        .flat_map(|r| [r.0, r.1])
        .chain(profile.iter().flat_map(|r| [r.0, r.1]))
        .collect();
    points.sort_unstable();
    points.dedup();
    (0usize..1usize << jobs.len())
        .filter_map(|bits| {
            for pair in points.windows(2) {
                let t = pair[0];
                let c = profile
                    .iter()
                    .find(|r| r.0 <= t && t < r.1)
                    .map_or(0, |r| r.2);
                let active = jobs
                    .iter()
                    .enumerate()
                    .filter(|&(i, &(s, e, _))| bits & (1 << i) != 0 && s <= t && t < e)
                    .count();
                if active > c {
                    return None;
                }
            }
            Some(
                jobs.iter()
                    .enumerate()
                    .filter(|&(i, _)| bits & (1 << i) != 0)
                    .map(|(_, r)| i128::from(r.2))
                    .sum(),
            )
        })
        .max()
        .unwrap()
}
