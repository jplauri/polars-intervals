#[path = "../benches/support/capacity_reference.rs"]
mod reference;

use intervals_core::{
    IntervalError, max_weight_non_overlapping, max_weight_with_capacity as solve,
};
use proptest::prelude::*;
use reference::{brute_force, verify};

fn check(s: &[i64], e: &[i64], w: &[i64], k: usize) -> (Vec<bool>, i128) {
    let expected = brute_force(s, e, w, k);
    let mask = solve(s, e, w, k).unwrap();
    assert_eq!(verify(s, e, w, k, &mask), expected);
    assert_eq!(mask, solve(s, e, w, k).unwrap());
    assert_eq!(verify(s, e, w, k, &reference::solve(s, e, w, k)), expected);
    (mask, expected)
}

#[test]
fn empty_zero_capacity_and_nonpositive() {
    for k in [0, 1, 2, 64, usize::MAX] {
        assert_eq!(check(&[], &[], &[], k), (vec![], 0));
        assert_eq!(check(&[0, 1], &[4, 1], &[-1, -2], k).0, [false; 2]);
        assert_eq!(
            check(&[0; 4], &[0, 0, 0, 4], &[3, -1, 0, 10], 0).0,
            [true, false, false, false]
        );
        assert_eq!(check(&[0, 0], &[0, 3], &[0, 0], k).0, [false; 2]);
    }
}

#[test]
fn disjoint_touching_and_sufficient_capacity() {
    for k in [1, 2, 64, usize::MAX] {
        assert_eq!(check(&[0, 1, 4], &[1, 2, 5], &[1, 2, 3], k).0, [true; 3]);
    }
    for k in [2, 3, 64, usize::MAX] {
        assert_eq!(
            check(&[0, 1, 2, 3], &[2, 3, 4, 5], &[3, 2, 2, 3], k).0,
            [true; 4]
        );
    }
}

#[test]
fn clique_duplicates_top_k_and_saturated_empties() {
    for ends in [[10; 5], [6, 7, 8, 9, 10]] {
        assert_eq!(
            check(&[0; 5], &ends, &[1, 50, 3, 40, 5], 2).0,
            [false, true, false, true, false]
        );
    }
    let s = [0, 0, 0, 0, 0, 3, 4, 4, 4, 4];
    let e = [10, 10, 10, 10, 10, 3, 4, 4, 4, 4];
    let w = [1, 50, 3, 40, 5, 8, 7, 6, -1, 0];
    assert_eq!(
        check(&s, &e, &w, 2).0,
        [
            false, true, false, true, false, true, true, true, false, false
        ]
    );
}

#[test]
fn nested_greedy_counterexample_and_shuffled_reconstruction() {
    let s = [0, 0, 4, 7, 1, 2];
    let e = [10, 4, 7, 10, 9, 3];
    let w = [15, 10, 10, 10, 25, 9];
    let (mask, _) = check(&s, &e, &w, 2);
    let order = [4, 1, 5, 0, 3, 2];
    let shuffled = check(
        &order.map(|i| s[i]),
        &order.map(|i| e[i]),
        &order.map(|i| w[i]),
        2,
    )
    .0;
    assert_eq!(shuffled, order.map(|i| mask[i]));
    // Descending weight picks the long 15 first, losing the schedule of 30.
    assert_eq!(check(&s[..4], &e[..4], &w[..4], 1).1, 30);
    assert_eq!(check(&s[..4], &e[..4], &w[..4], 2).1, 45);
    assert_eq!(
        check(&[0, 1, 2, 3, 4], &[10, 9, 8, 7, 6], &[9, 20, 2, 17, 6], 2).1,
        37
    );
}

#[test]
fn repeated_single_resource_is_suboptimal() {
    // Unique best first schedule is A+D (6), leaving overlapping B+C.
    // Repeated scheduling gets 8, while capacity two selects A+B+C+D (10).
    // The low-value spanning row makes candidate concurrency three, forcing
    // the general solver instead of the sufficient-capacity fast path.
    let (s, e, w) = ([0, 1, 2, 3, 0], [2, 3, 4, 5, 5], [3, 2, 2, 3, 1]);
    let first = max_weight_non_overlapping(&s, &e, &w).unwrap();
    assert_eq!(first, [true, false, false, true, false]);
    let remaining: Vec<_> = w
        .iter()
        .zip(&first)
        .map(|(&w, &v)| if v { 0 } else { w })
        .collect();
    let second = max_weight_non_overlapping(&s, &e, &remaining).unwrap();
    let repeated = verify(&s, &e, &w, 1, &first) + verify(&s, &e, &remaining, 1, &second);
    assert_eq!(repeated, 8);
    assert_eq!(check(&s, &e, &w, 2).1, 10);
}

#[test]
fn independent_components() {
    assert_eq!(
        check(
            &[0, 0, 0, 10, 10, 10],
            &[2, 2, 2, 12, 12, 12],
            &[5, 8, 2, 4, 1, 9],
            2
        )
        .1,
        26
    );
}

#[test]
fn parallel_component_batches_preserve_order_and_exact_top_k() {
    let mut s: Vec<_> = (0..8192).map(|i| i / 32 * 100).collect();
    let mut e: Vec<_> = s.iter().map(|s| s + 10).collect();
    let mut w: Vec<_> = (0..8192).map(|i| i % 32 + 1).collect();
    s.reverse();
    e.reverse();
    w.reverse();
    let expected: Vec<_> = w.iter().map(|&w| w > 24).collect();
    for _ in 0..3 {
        assert_eq!(solve(&s, &e, &w, 8).unwrap(), expected);
    }
}

#[test]
fn validation_and_integer_boundaries() {
    for k in [0, 1, 2, usize::MAX] {
        assert_eq!(
            solve(&[0], &[], &[1], k),
            Err(IntervalError::LengthMismatch {
                starts_len: 1,
                ends_len: 0
            })
        );
        for weights in [vec![], vec![1, 2]] {
            assert_eq!(
                solve(&[0], &[1], &weights, k),
                Err(IntervalError::WeightLengthMismatch {
                    intervals_len: 1,
                    weights_len: weights.len()
                })
            );
        }
        assert_eq!(
            solve(&[0, 4, 8], &[1, 3, 7], &[1, -2, 0], k),
            Err(IntervalError::InvalidInterval { index: 1 })
        );
    }
    assert_eq!(
        solve(&[i64::MIN, 0], &[0, i64::MAX], &[u64::MAX; 2], 2).unwrap(),
        [true; 2]
    );
    assert_eq!(
        solve(&[0; 3], &[1; 3], &[u64::MAX, u64::MAX - 1, 1], 2).unwrap(),
        [true, true, false]
    );
    assert_eq!(
        solve(&[0; 3], &[1; 3], &[i64::MAX, i64::MIN, i64::MAX], 2).unwrap(),
        [true, false, true]
    );
    assert_eq!(
        solve(&['a'; 3], &['z'; 3], &[1, 2, 3], 2).unwrap(),
        [false, true, true]
    );
    for (s, e, w, k) in [
        (vec![0, 0], vec![0, 0], vec![i128::MAX, 1], 0),
        (vec![0, 0], vec![1, 1], vec![i128::MAX, 1], 2),
        (vec![0; 3], vec![1; 3], vec![i128::MAX, 1, 1], 2),
        (
            vec![0, 0, 0, 2, 2, 2],
            vec![1, 1, 1, 3, 3, 3],
            vec![i128::MAX / 2; 6],
            2,
        ),
    ] {
        assert_eq!(solve(&s, &e, &w, k), Err(IntervalError::WeightOverflow));
    }
    assert_eq!(
        solve(&[0; 3], &[1; 3], &[i128::MAX - 1, 1, 1], 2)
            .unwrap()
            .iter()
            .filter(|&&v| v)
            .count(),
        2
    );
}

fn instances() -> impl Strategy<Value = Vec<(i64, i64, i64)>> {
    prop::collection::vec((-4i64..=4, 0i64..=6, -10i64..=20), 0..=11).prop_map(|rows| {
        rows.into_iter()
            .map(|(s, len, w)| (s, s + len, w))
            .collect()
    })
}

fn columns(rows: &[(i64, i64, i64)]) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

proptest! {
    #[test]
    fn feasibility_optimality_length_determinism_and_monotonicity(rows in instances()) {
        let (s, e, w) = columns(&rows);
        let mut previous = 0;
        for k in 0..=6 {
            let (mask, objective) = check(&s, &e, &w, k);
            prop_assert_eq!(mask.len(), s.len());
            prop_assert!(objective >= previous);
            previous = objective;
            if k == 1 {
                prop_assert_eq!(mask, max_weight_non_overlapping(&s, &e, &w).unwrap());
            }
        }
    }

    #[test]
    fn sufficient_capacity_and_beyond_n(rows in instances()) {
        let (s, e, w) = columns(&rows);
        let peak = s.iter().map(|&t| (0..s.len()).filter(|&i| w[i] > 0 && s[i] <= t && t < e[i]).count()).max().unwrap_or(0);
        let expected: i128 = w.iter().map(|&w| i128::from(w.max(0))).sum();
        let (mask, objective) = check(&s, &e, &w, peak);
        prop_assert_eq!(objective, expected);
        prop_assert_eq!(&mask, &solve(&s, &e, &w, s.len() + 10).unwrap());
        prop_assert_eq!(mask, solve(&s, &e, &w, usize::MAX).unwrap());
    }

    #[test]
    fn permutation_and_translation(mut rows in instances(), k in 0usize..=5, offset in -100i64..=100, seed in any::<u64>()) {
        let (s, e, w) = columns(&rows);
        let (mask, objective) = check(&s, &e, &w, k);
        let ss: Vec<_> = s.iter().map(|s| s + offset).collect();
        let ee: Vec<_> = e.iter().map(|e| e + offset).collect();
        prop_assert_eq!(check(&ss, &ee, &w, k), (mask, objective));
        let mut state = seed;
        for i in (1..rows.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            rows.swap(i, (state % (i + 1) as u64) as usize);
        }
        let (s, e, w) = columns(&rows);
        prop_assert_eq!(check(&s, &e, &w, k).1, objective);
    }

    #[test]
    fn negative_row_and_positive_empty(rows in instances(), k in 0usize..=5, x in -5i64..=5, length in 0i64..=6, weight in 1i64..=20) {
        let (mut s, mut e, mut w) = columns(&rows);
        let (_, objective) = check(&s, &e, &w, k);
        s.push(x); e.push(x + length); w.push(-weight);
        let (mask, after) = check(&s, &e, &w, k);
        prop_assert_eq!(after, objective);
        prop_assert!(!mask[mask.len() - 1]);
        e[s.len() - 1] = x; w[s.len() - 1] = weight;
        prop_assert_eq!(check(&s, &e, &w, k).1, objective + i128::from(weight));
    }

    #[test]
    fn clique_top_k(rows in prop::collection::vec((0i64..=5, 1i64..=5, -10i64..=20), 0..=8), empties in prop::collection::vec(-10i64..=20, 0..=3), k in 0usize..=5) {
        let mut s: Vec<_> = rows.iter().map(|r| -r.0).collect();
        let mut e: Vec<_> = rows.iter().map(|r| r.1).collect();
        let mut w: Vec<_> = rows.iter().map(|r| r.2).collect();
        let mut positive: Vec<_> = w.iter().copied().filter(|&w| w > 0).collect();
        positive.sort_unstable_by(|a,b| b.cmp(a));
        let expected: i128 = positive.into_iter().take(k).map(i128::from).sum::<i128>()
            + empties.iter().map(|&w| i128::from(w.max(0))).sum::<i128>();
        for empty in empties { s.push(0); e.push(0); w.push(empty); }
        prop_assert_eq!(check(&s, &e, &w, k).1, expected);
    }

    #[test]
    fn component_additivity(a in instances(), b in instances(), k in 0usize..=5) {
        let (sa, ea, wa) = columns(&a);
        let (sb, eb, wb) = columns(&b);
        let expected = check(&sa, &ea, &wa, k).1 + check(&sb, &eb, &wb, k).1;
        let mut s = sa; s.extend(sb.iter().map(|s| s + 30));
        let mut e = ea; e.extend(eb.iter().map(|e| e + 30));
        let mut w = wa; w.extend(wb);
        let mask = solve(&s, &e, &w, k).unwrap();
        prop_assert_eq!(verify(&s, &e, &w, k, &mask), expected);
        prop_assert_eq!(verify(&s, &e, &w, k, &reference::solve(&s, &e, &w, k)), expected);
    }
}
