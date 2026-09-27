use intervals_core::{
    IntervalError, max_weight_non_overlapping, max_weight_with_capacity,
    max_weight_with_capacity_profile as solve,
};
use proptest::prelude::*;

type Row = (i64, i64, i64);

fn columns(rows: &[Row]) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

fn selected(jobs: &[Row], profile: &[Row]) -> Vec<bool> {
    let (s, e, w) = columns(jobs);
    let (ps, pe, c) = columns(profile);
    solve(&s, &e, &w, &ps, &pe, &c).unwrap()
}

fn objective(jobs: &[Row], mask: &[bool]) -> i128 {
    assert_eq!(jobs.len(), mask.len());
    jobs.iter()
        .zip(mask)
        .filter(|(_, keep)| **keep)
        .map(|(job, _)| i128::from(job.2))
        .sum()
}

// Deliberately independent of normalization, compression, and flow code. Every
// endpoint from BOTH original tables participates in the atomic segment check.
fn feasible(jobs: &[Row], profile: &[Row], mask: &[bool]) -> bool {
    assert_eq!(jobs.len(), mask.len());
    let mut points: Vec<_> = jobs
        .iter()
        .chain(profile)
        .flat_map(|r| [r.0, r.1])
        .collect();
    points.sort_unstable();
    points.dedup();
    points.windows(2).all(|pair| {
        let t = pair[0];
        let available = profile
            .iter()
            .find(|r| r.0 <= t && t < r.1)
            .map_or(0, |r| r.2);
        let active = jobs
            .iter()
            .zip(mask)
            .filter(|(r, keep)| **keep && r.0 <= t && t < r.1)
            .count();
        active as i64 <= available
    })
}

fn brute_force(jobs: &[Row], profile: &[Row]) -> i128 {
    assert!(jobs.len() <= 12);
    (0..1usize << jobs.len())
        .filter_map(|bits| {
            let mask: Vec<_> = (0..jobs.len()).map(|i| bits & (1 << i) != 0).collect();
            feasible(jobs, profile, &mask).then(|| objective(jobs, &mask))
        })
        .max()
        .unwrap()
}

fn check(jobs: &[Row], profile: &[Row]) -> (Vec<bool>, i128) {
    let mask = selected(jobs, profile);
    assert!(feasible(jobs, profile, &mask));
    assert_eq!(mask, selected(jobs, profile));
    assert!(jobs.iter().zip(&mask).all(|(r, keep)| !keep || r.2 > 0));
    let value = objective(jobs, &mask);
    assert_eq!(value, brute_force(jobs, profile));
    (mask, value)
}

fn opt(jobs: &[Row], profile: &[Row]) -> i128 {
    let mask = selected(jobs, profile);
    assert!(feasible(jobs, profile, &mask));
    objective(jobs, &mask)
}

#[test]
fn empty_jobs_empty_profile_positive_empties_and_nonpositive_ties() {
    assert_eq!(check(&[], &[]), (vec![], 0));
    assert_eq!(check(&[], &[(0, 10, 3)]), (vec![], 0));
    let jobs = [(0, 10, 100), (3, 3, 7), (4, 4, 0), (5, 5, -4)];
    for profile in [vec![], vec![(0, 10, 0)], vec![(3, 3, 100)]] {
        assert_eq!(check(&jobs, &profile), (vec![false, true, false, false], 7));
    }
}

#[test]
fn constant_profile_matches_scalar_capacity_and_specialized_one() {
    let jobs = [(0, 10, 15), (0, 4, 10), (4, 7, 10), (7, 10, 10), (3, 3, 4)];
    let (s, e, w) = columns(&jobs);
    for k in [0, 1, 2, 3, 64] {
        let expected = max_weight_with_capacity(&s, &e, &w, k as usize).unwrap();
        assert_eq!(check(&jobs, &[(0, 100, k)]).1, objective(&jobs, &expected));
    }
    assert_eq!(
        check(&jobs, &[(0, 100, 1)]).1,
        objective(&jobs, &max_weight_non_overlapping(&s, &e, &w).unwrap())
    );
}

#[test]
fn explicit_and_implicit_zero_gap_forbid_crossing_jobs() {
    let jobs = [(0, 5, 8), (7, 12, 9), (4, 8, 100), (5, 7, 200), (6, 6, 3)];
    let expected = (vec![true, true, false, false, true], 20);
    assert_eq!(check(&jobs, &[(0, 5, 2), (5, 7, 0), (7, 12, 2)]), expected);
    assert_eq!(check(&jobs, &[(0, 5, 2), (7, 12, 2)]), expected);
}

#[test]
fn bottleneck_and_huge_capacity_outside_do_not_rescue_long_jobs() {
    let jobs = [
        (0, 12, 100),
        (1, 11, 80),
        (2, 10, 70),
        (0, 4, 9),
        (8, 12, 8),
    ];
    for outer in [3, 1_000_000, i64::MAX] {
        assert_eq!(
            check(&jobs, &[(0, 4, outer), (4, 8, 1), (8, 12, outer)]),
            (vec![true, false, false, true, true], 117)
        );
    }
}

#[test]
fn capacity_increases_and_decreases_at_touching_boundaries() {
    let jobs = [(0, 10, 20), (0, 10, 15), (0, 5, 9), (5, 10, 8), (5, 10, 7)];
    assert_eq!(check(&jobs, &[(0, 5, 1), (5, 10, 3)]).1, 35);
    assert_eq!(check(&jobs, &[(0, 5, 3), (5, 10, 1)]).1, 29);
    assert_eq!(check(&jobs, &[(0, 5, 2), (5, 10, 3)]).1, 44);
}

#[test]
fn unsorted_coalesced_profile_ignores_valid_zero_length_rows() {
    let jobs = [(0, 10, 7), (0, 5, 5), (5, 10, 6), (4, 7, 3)];
    let expected = check(&jobs, &[(0, 10, 2)]);
    for profile in [
        vec![(0, 5, 2), (5, 10, 2)],
        vec![(5, 10, 2), (5, 5, 100), (0, 5, 2)],
        vec![(9, 9, 0), (0, 10, 2), (0, 0, 4)],
    ] {
        assert_eq!(check(&jobs, &profile), expected);
    }
}

#[test]
fn clique_selects_top_k_and_output_keeps_original_row_order() {
    let jobs = [(0, 12, 3), (0, 12, 50), (0, 12, 8), (0, 12, 40), (0, 12, 6)];
    let profile = [(0, 4, 8), (4, 8, 2), (8, 12, 8)];
    assert_eq!(
        check(&jobs, &profile),
        (vec![false, true, false, true, false], 90)
    );
    let order = [4, 1, 3, 0, 2];
    assert_eq!(
        check(&order.map(|i| jobs[i]), &profile),
        (vec![false, true, true, false, false], 90)
    );
}

#[test]
fn weight_greedy_loses_to_short_jobs_around_bottleneck() {
    // Weight-first accepts the long 15 and blocks both 10s at capacity one.
    // The global optimum accepts the touching short pair, worth 20.
    let jobs = [(0, 10, 15), (0, 5, 10), (5, 10, 10)];
    assert_eq!(
        check(&jobs, &[(0, 4, 3), (4, 6, 1), (6, 10, 3)]),
        (vec![false, true, true], 20)
    );
}

#[test]
fn outside_profile_is_zero_and_touching_the_last_endpoint_is_empty() {
    let jobs = [
        (-1, 4, 100),
        (1, 11, 200),
        (-4, -1, 20),
        (0, 10, 7),
        (10, 10, 3),
    ];
    assert_eq!(
        check(&jobs, &[(0, 10, 100)]),
        (vec![false, false, false, true, true], 10)
    );
}

#[test]
fn all_positive_feasible_and_capacity_clamping() {
    let jobs = [(0, 4, 3), (1, 5, 4), (4, 8, 9), (4, 6, -3), (12, 12, 7)];
    for cap in [3, 4, i64::MAX] {
        assert_eq!(
            check(&jobs, &[(0, 4, cap), (4, 8, 2)]),
            (vec![true, true, true, false, true], 23)
        );
    }
}

#[test]
fn validation_is_not_skipped_for_empty_inputs_or_irrelevant_rows() {
    assert!(solve(&[0], &[], &[1], &[], &[], &[] as &[i64]).is_err());
    assert!(solve(&[0], &[1], &[] as &[i64], &[], &[], &[] as &[i64]).is_err());
    for jobs in [vec![], vec![(0, 0, 1)], vec![(0, 2, -1)]] {
        let (s, e, w) = columns(&jobs);
        assert!(solve(&s, &e, &w, &[0], &[], &[0]).is_err());
        assert!(solve(&s, &e, &w, &[0], &[1], &[] as &[i64]).is_err());
        for invalid in [
            vec![(7, 5, 0)],
            vec![(5, 5, -1)],
            vec![(0, 7, 2), (5, 10, 3)],
        ] {
            let (ps, pe, c) = columns(&invalid);
            assert!(solve(&s, &e, &w, &ps, &pe, &c).is_err());
        }
    }
    assert_eq!(
        solve(&[2], &[1], &[-1], &[], &[], &[] as &[i64]),
        Err(IntervalError::InvalidInterval { index: 0 })
    );
}

#[test]
fn integer_extremes_generic_endpoints_and_checked_objective() {
    assert_eq!(
        solve(
            &[0u64, u64::MAX - 1, 0],
            &[u64::MAX, u64::MAX, 1u64 << 63],
            &[u64::MAX, u64::MAX - 1, u64::MAX - 2],
            &[0u64, 1u64 << 63],
            &[1u64 << 63, u64::MAX],
            &[2, 1]
        )
        .unwrap(),
        [true, false, true]
    );
    assert_eq!(
        solve(
            &[i64::MIN, 0],
            &[0, i64::MAX],
            &[u64::MAX; 2],
            &[i64::MIN, 0],
            &[0, i64::MAX],
            &[u64::MAX, 1]
        )
        .unwrap(),
        [true, true]
    );
    assert_eq!(
        solve(&['a', 'a'], &['z', 'z'], &[1, 2], &['a'], &['z'], &[1]).unwrap(),
        [false, true]
    );
    assert_eq!(
        solve(&[0, 0], &[0, 0], &[i128::MAX, 1], &[], &[], &[] as &[u64]),
        Err(IntervalError::WeightOverflow)
    );
    assert_eq!(
        solve(
            &[0, 0, 1],
            &[3, 2, 3],
            &[i128::MAX, 1, 1],
            &[0, 1],
            &[1, 3],
            &[3, 2]
        ),
        Err(IntervalError::WeightOverflow)
    );
}

fn jobs() -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec((-6i64..=10, -6i64..=10, -10i64..=20), 0..=10).prop_map(|v| {
        v.into_iter()
            .map(|(a, b, w)| (a.min(b), a.max(b), w))
            .collect()
    })
}

fn capacities() -> impl Strategy<Value = Vec<i64>> {
    prop::collection::vec(0i64..=4, 16)
}

fn units(cap: &[i64], omit_zero: bool) -> Vec<Row> {
    cap.iter()
        .enumerate()
        .filter(|(_, c)| !omit_zero || **c != 0)
        .map(|(i, &c)| (i as i64 - 6, i as i64 - 5, c))
        .collect()
}

fn coalesce(rows: &[Row]) -> Vec<Row> {
    let mut result: Vec<Row> = Vec::new();
    for &r in rows {
        if let Some(last) = result.last_mut()
            && last.1 == r.0
            && last.2 == r.2
        {
            last.1 = r.1;
            continue;
        }
        result.push(r);
    }
    result
}

fn permute<T>(rows: &mut [T], mut state: u64) {
    for i in (1..rows.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        rows.swap(i, (state % (i + 1) as u64) as usize);
    }
}

proptest! {
    #[test]
    fn exact_optimum_feasibility_length_and_determinism(rows in jobs(), caps in capacities()) {
        let profile = coalesce(&units(&caps, true));
        let (mask, _) = check(&rows, &profile);
        prop_assert_eq!(mask.len(), rows.len());
    }

    #[test]
    fn constant_and_capacity_one_equivalence(rows in jobs(), k in 0usize..=4) {
        let (s, e, w) = columns(&rows);
        prop_assert_eq!(opt(&rows, &[(-6, 10, k as i64)]),
            objective(&rows, &max_weight_with_capacity(&s, &e, &w, k).unwrap()));
        prop_assert_eq!(opt(&rows, &[(-6, 10, 1)]),
            objective(&rows, &max_weight_non_overlapping(&s, &e, &w).unwrap()));
    }

    #[test]
    fn pointwise_and_single_segment_capacity_monotonicity(
        rows in jobs(), caps in capacities(), increments in capacities(), index in 0usize..16,
    ) {
        let base = opt(&rows, &units(&caps, false));
        let greater: Vec<_> = caps.iter().zip(&increments).map(|(a, b)| a + b).collect();
        prop_assert!(opt(&rows, &units(&greater, false)) >= base);
        let mut changed = caps.clone();
        changed[index] += 1;
        prop_assert!(opt(&rows, &units(&changed, false)) >= base);
        changed[index] = caps[index].saturating_sub(1).max(0);
        prop_assert!(opt(&rows, &units(&changed, false)) <= base);
    }

    #[test]
    fn splitting_coalescing_and_implicit_zero_gaps(rows in jobs(), caps in capacities()) {
        let explicit = units(&caps, false);
        let implicit = units(&caps, true);
        let value = opt(&rows, &explicit);
        prop_assert_eq!(value, opt(&rows, &implicit));
        prop_assert_eq!(value, opt(&rows, &coalesce(&explicit)));
        prop_assert_eq!(value, opt(&rows, &coalesce(&implicit)));
        // Scaling coordinates permits splitting every unit segment internally.
        let scaled_jobs: Vec<_> = rows.iter().map(|r| (r.0 * 2, r.1 * 2, r.2)).collect();
        let split: Vec<_> = explicit.iter().flat_map(|r|
            [(r.0 * 2, r.0 * 2 + 1, r.2), (r.0 * 2 + 1, r.1 * 2, r.2)]).collect();
        prop_assert_eq!(value, opt(&scaled_jobs, &split));
    }

    #[test]
    fn permutation_and_translation_invariance(mut rows in jobs(), caps in capacities(), seed in any::<u64>(), offset in -100i64..=100) {
        let mut profile = coalesce(&units(&caps, true));
        let original = selected(&rows, &profile);
        let value = objective(&rows, &original);
        permute(&mut profile, seed);
        prop_assert_eq!(selected(&rows, &profile), original);
        permute(&mut rows, seed.wrapping_add(17));
        prop_assert_eq!(opt(&rows, &profile), value);
        let shifted_jobs: Vec<_> = rows.iter().map(|r| (r.0 + offset, r.1 + offset, r.2)).collect();
        let shifted_profile: Vec<_> = profile.iter().map(|r| (r.0 + offset, r.1 + offset, r.2)).collect();
        prop_assert_eq!(opt(&shifted_jobs, &shifted_profile), value);
    }

    #[test]
    fn all_positive_feasible_zero_profile_and_clamping(rows in jobs(), caps in capacities()) {
        let profile = units(&caps, true);
        let positive: Vec<_> = rows.iter().map(|r| r.2 > 0).collect();
        if feasible(&rows, &profile, &positive) {
            prop_assert_eq!(opt(&rows, &profile), objective(&rows, &positive));
        }
        let zero = selected(&rows, &[]);
        prop_assert_eq!(&zero, &rows.iter().map(|r| r.0 == r.1 && r.2 > 0).collect::<Vec<_>>());
        let n = rows.iter().filter(|r| r.0 < r.1 && r.2 > 0).count() as i64;
        let clamped: Vec<_> = profile.iter().map(|r| (r.0, r.1, r.2.min(n))).collect();
        prop_assert_eq!(opt(&rows, &profile), opt(&rows, &clamped));
    }

    #[test]
    fn positive_empty_and_negative_job_extension(mut rows in jobs(), caps in capacities(), location in -20i64..=20, weight in 1i64..=20) {
        let profile = units(&caps, true);
        let base = opt(&rows, &profile);
        rows.push((location, location, weight));
        prop_assert_eq!(opt(&rows, &profile), base + i128::from(weight));
        rows.push((-6, 10, -weight));
        let mask = selected(&rows, &profile);
        prop_assert!(!mask.last().unwrap());
        prop_assert_eq!(objective(&rows, &mask), base + i128::from(weight));
    }

    #[test]
    fn separated_regions_are_additive(left in jobs(), right in jobs(), lc in capacities(), rc in capacities()) {
        let lp = units(&lc, true);
        let right: Vec<_> = right.iter().map(|r| (r.0 + 30, r.1 + 30, r.2)).collect();
        let rp: Vec<_> = units(&rc, true).iter().map(|r| (r.0 + 30, r.1 + 30, r.2)).collect();
        let expected = opt(&left, &lp) + opt(&right, &rp);
        let combined_jobs: Vec<_> = left.into_iter().chain(right).collect();
        let combined_profile: Vec<_> = lp.into_iter().chain(rp).collect();
        prop_assert_eq!(opt(&combined_jobs, &combined_profile), expected);
    }
}
