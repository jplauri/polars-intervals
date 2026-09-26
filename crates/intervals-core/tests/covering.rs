use intervals_core::{IntervalError, minimum_cost_cover, minimum_cover};
use proptest::prelude::*;

#[path = "../benches/support/covering.rs"]
mod candidates;
use candidates::{brute_force, covers, objective};

fn check(s: &[i32], e: &[i32], w: &[i128], l: i32, r: i32) -> Option<(i128, usize)> {
    let expected = brute_force(s, e, w, l, r);
    let actual = minimum_cost_cover(s, e, w, l, r);
    match expected {
        Some(optimum) => {
            let mask = actual.unwrap();
            assert_eq!(mask.len(), s.len());
            assert!(covers(s, e, &mask, l, r));
            assert_eq!(objective(w, &mask), optimum);
            assert!(mask.iter().enumerate().all(|(i, &m)| !m || s[i] < e[i]));
            assert_eq!(minimum_cost_cover(s, e, w, l, r).unwrap(), mask);
        }
        None => assert_eq!(actual, Err(IntervalError::InfeasibleCover)),
    }
    for method in ["MCC-A", "MCC-B", "MCC-C"] {
        let actual = candidates::weighted(s, e, w, l, r, method).result;
        assert_eq!(
            actual.as_ref().ok().map(|mask| objective(w, mask)),
            expected
        );
        if let Ok(mask) = actual {
            assert!(covers(s, e, &mask, l, r));
        }
    }
    let unit = vec![1; s.len()];
    let cardinality = brute_force(s, e, &unit, l, r);
    let greedy = minimum_cover(s, e, l, r);
    assert_eq!(
        greedy.as_ref().ok().map(|mask| objective(&unit, mask)),
        cardinality
    );
    assert_eq!(greedy, minimum_cover(s, e, l, r));
    if let Ok(mask) = &greedy {
        assert_eq!(mask.len(), s.len());
        assert!(covers(s, e, mask, l, r));
        let weighted = minimum_cost_cover(s, e, &unit, l, r).unwrap();
        assert_eq!(objective(&unit, mask), objective(&unit, &weighted));
    }
    for method in ["MC-A", "MC-B", "MC-C", "MC-A-detect"] {
        let actual = candidates::cardinality(s, e, l, r, method).result;
        assert_eq!(
            actual.as_ref().ok().map(|mask| objective(&unit, mask)),
            cardinality
        );
        if let Ok(mask) = actual {
            assert!(covers(s, e, &mask, l, r));
        }
    }
    expected
}

fn interval() -> impl Strategy<Value = (i32, i32)> {
    (-8i32..=12, -8i32..=12).prop_map(|(a, b)| (a.min(b), a.max(b)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn exhaustive_objectives_and_metamorphic_properties(
        rows in prop::collection::vec((interval(), 0i128..=20, any::<u32>()), 0..=11),
        (l, r) in interval(), (add_l, add_r) in interval(), add_cost in 0i128..=20,
        offset in -100i32..=100, scale in 1i128..=20,
    ) {
        let s: Vec<_> = rows.iter().map(|row| row.0.0).collect();
        let e: Vec<_> = rows.iter().map(|row| row.0.1).collect();
        let w: Vec<_> = rows.iter().map(|row| row.1).collect();
        let optimum = check(&s, &e, &w, l, r);
        let unit = vec![1; s.len()];
        let count = minimum_cover(&s, &e, l, r).ok().map(|m| objective(&unit, &m).1);
        let shifted_s: Vec<_> = s.iter().map(|x| x + offset).collect();
        let shifted_e: Vec<_> = e.iter().map(|x| x + offset).collect();
        prop_assert_eq!(check(&shifted_s, &shifted_e, &w, l + offset, r + offset), optimum);
        let mut permutation: Vec<_> = (0..s.len()).collect();
        permutation.sort_by_key(|&i| rows[i].2);
        let ps: Vec<_> = permutation.iter().map(|&i| s[i]).collect();
        let pe: Vec<_> = permutation.iter().map(|&i| e[i]).collect();
        let pw: Vec<_> = permutation.iter().map(|&i| w[i]).collect();
        prop_assert_eq!(check(&ps, &pe, &pw, l, r), optimum);
        let scaled: Vec<_> = w.iter().map(|x| x * scale).collect();
        prop_assert_eq!(check(&s, &e, &scaled, l, r), optimum.map(|(c,n)| (c * scale,n)));
        // All-zero costs give exactly the minimum-cardinality cover.
        prop_assert_eq!(check(&s, &e, &vec![0; s.len()], l, r), count.map(|n| (0,n)));
        let (mut more_s, mut more_e, mut more_w) = (s.clone(), e.clone(), w.clone());
        more_s.push(add_l); more_e.push(add_r); more_w.push(add_cost);
        if let Some(optimum) = optimum {
            let mask = minimum_cost_cover(&more_s, &more_e, &more_w, l, r).unwrap();
            prop_assert!(objective(&more_w, &mask) <= optimum);
            let mask = minimum_cover(&more_s, &more_e, l, r).unwrap();
            prop_assert!(mask.iter().filter(|&&m| m).count() <= count.unwrap());
        }
        more_s[s.len()] = r + 1; more_e[s.len()] = r + 2;
        prop_assert_eq!(check(&more_s, &more_e, &more_w, l, r), optimum);
        if l < r && (0..s.len()).any(|i| s[i] <= l && e[i] >= r) {
            prop_assert_eq!(count, Some(1));
        }
        // Every published state/backpointer is itself an exact continuous prefix cover.
        let mut prepared = candidates::production::prepare(&s, &e, l, r).unwrap();
        prepared.sort_unstable_by_key(|c| (c.end, c.row));
        let coordinates = candidates::production::coordinates(&prepared, l);
        let solution = candidates::production::dynamic_program(&prepared, &coordinates, &w);
        for (i, &end) in coordinates.iter().enumerate().skip(1) {
            let mut mask = vec![false; s.len()];
            let reachable = candidates::production::reconstruct(&solution.back[..=i], &mut mask);
            let oracle = brute_force(&s, &e, &w, l, end);
            // Prefix covers may use intervals ending later; the DP at i only uses ends <= i.
            let allowed_s: Vec<_> = prepared.iter().filter(|c| c.end <= end).map(|c| c.start).collect();
            let allowed_e: Vec<_> = prepared.iter().filter(|c| c.end <= end).map(|c| c.end).collect();
            let allowed_w: Vec<_> = prepared.iter().filter(|c| c.end <= end).map(|c| w[c.row]).collect();
            let prefix = brute_force(&allowed_s, &allowed_e, &allowed_w, l, end);
            prop_assert_eq!(reachable, prefix.is_some());
            if reachable {
                prop_assert!(oracle.is_some());
                prop_assert!(covers(&s, &e, &mask, l, end));
                prop_assert_eq!(Some(objective(&w, &mask)), prefix);
                prop_assert!(solution.back[i].unwrap().0 < i);
            }
        }
    }

    #[test]
    fn trees_match_naive_ranges(values in prop::collection::vec(prop::option::of((0i128..100, 0usize..10)), 1..40)) {
        let mut fenwick = candidates::production::Fenwick::new(values.len());
        let mut segment = candidates::SegmentTree::new(values.len());
        let mut naive = vec![None; values.len()];
        for (i, value) in values.iter().enumerate() {
            naive[i] = value.map(|(cost, count)| (cost, count, i));
            fenwick.update(i, naive[i]); segment.update(i, naive[i]);
            for l in 0..values.len() {
                let expected = naive[l..].iter().flatten().copied().min();
                prop_assert_eq!(fenwick.suffix(l), expected);
                prop_assert_eq!(segment.query(l, values.len()), expected);
                for r in l..=values.len() {
                    prop_assert_eq!(segment.query(l, r), naive[l..r].iter().flatten().copied().min());
                }
            }
        }
    }
}

#[test]
fn deterministic_regressions() {
    for (s, e, w, l, r) in [
        (vec![], vec![], vec![], 5, 5),
        (vec![0], vec![10], vec![1], 0, 10),
        (vec![-100], vec![100], vec![1], 0, 10),
        (vec![0, 3, 7], vec![3, 7, 10], vec![1, 1, 1], 0, 10),
        (vec![0, 5], vec![4, 10], vec![1, 1], 0, 10),
        (
            vec![0, 0, 4, 6, 7],
            vec![4, 6, 7, 10, 10],
            vec![1; 5],
            0,
            10,
        ),
        (vec![0, 0, 5], vec![10, 5, 10], vec![100, 10, 10], 0, 10),
        (
            vec![0, 0, 4, 6],
            vec![4, 6, 10, 10],
            vec![1, 5, 100, 5],
            0,
            10,
        ),
        (
            vec![0, 0, 0, 5],
            vec![10, 10, 5, 10],
            vec![0, 0, 0, 0],
            0,
            10,
        ),
        (vec![0, 0], vec![10, 10], vec![9, 1], 0, 10),
        (
            vec![7, -1, 3, 2, 10],
            vec![12, 3, 7, 2, 10],
            vec![2; 5],
            0,
            10,
        ),
        (vec![0, 5, 10], vec![0, 5, 10], vec![0; 3], 0, 10),
        (vec![1, 2], vec![2, 3], vec![1; 2], 0, 3),
        (vec![0, 1], vec![1, 2], vec![1; 2], 0, 3),
    ] {
        check(&s, &e, &w, l, r);
    }
    assert_eq!(
        minimum_cover(&[0, 0, 4, 6, 7], &[4, 6, 7, 10, 10], 0, 10).unwrap(),
        [false, true, false, true, false]
    );
    assert_eq!(
        minimum_cover(&[0, 0], &[10, 10], 0, 10).unwrap(),
        [true, false]
    );
    assert_eq!(
        minimum_cost_cover(&[0, 0, 5], &[10, 5, 10], &[0, 0, 0], 0, 10).unwrap(),
        [true, false, false]
    );
    assert_eq!(
        minimum_cover(&[i128::MIN, 0], &[0, i128::MAX], i128::MIN, i128::MAX).unwrap(),
        [true, true]
    );
    assert_eq!(
        minimum_cover(&[0u64], &[u64::MAX], 0, u64::MAX).unwrap(),
        [true]
    );
}

#[test]
fn validation_and_overflow() {
    assert_eq!(
        minimum_cover(&['a', 'c'], &['c', 'z'], 'a', 'z').unwrap(),
        [true, true]
    );
    assert_eq!(
        minimum_cover(&[1], &[], 0, 1),
        Err(IntervalError::LengthMismatch {
            starts_len: 1,
            ends_len: 0
        })
    );
    assert_eq!(
        minimum_cover(&[0], &[1], 2, 1),
        Err(IntervalError::InvalidTarget)
    );
    assert_eq!(
        minimum_cover(&[2], &[1], 0, 0),
        Err(IntervalError::InvalidInterval { index: 0 })
    );
    assert_eq!(
        minimum_cost_cover(&[0], &[1], &[-1], 0, 0),
        Err(IntervalError::NegativeCost { index: 0 })
    );
    assert_eq!(
        minimum_cost_cover(&[0], &[1], &[] as &[i32], 0, 1),
        Err(IntervalError::CostLengthMismatch {
            intervals_len: 1,
            costs_len: 0
        })
    );
    assert_eq!(
        minimum_cost_cover(&[0, 1], &[1, 2], &[i128::MAX, 1], 0, 2),
        Err(IntervalError::CostOverflow)
    );
    assert_eq!(
        minimum_cost_cover(&[0, 1], &[1, 2], &[i128::MAX, 1], 0, 3),
        Err(IntervalError::InfeasibleCover)
    );
    assert_eq!(
        minimum_cost_cover(&[0, 1, 0], &[1, 2, 2], &[i128::MAX, 1, 5], 0, 2).unwrap(),
        [false, false, true]
    );
    assert_eq!(
        minimum_cost_cover(&[0, 1], &[1, 2], &[u64::MAX, u64::MAX], 0, 2).unwrap(),
        [true, true]
    );
    assert_eq!(
        minimum_cost_cover(&[0, 1], &[1, 2], &[i128::MAX - 1, 1], 0, 2).unwrap(),
        [true, true]
    );
}
