#[path = "../benches/support/weighted.rs"]
mod weighted;

use intervals_core::{IntervalError, max_weight_non_overlapping as solve};
use proptest::prelude::*;
use weighted::{CANDIDATES, shuffle, suffix_optimum, verify};

// Deliberately independent: enumerate every subset and every selected pair.
fn brute_force(s: &[i64], e: &[i64], w: &[i64]) -> i128 {
    let mut best = 0;
    for bits in 0usize..1 << s.len() {
        let mut valid = true;
        let mut weight = 0i128;
        for i in 0..s.len() {
            if bits & (1 << i) == 0 {
                continue;
            }
            weight += i128::from(w[i]);
            for j in 0..i {
                if bits & (1 << j) != 0 && s[i] < e[i] && s[j] < e[j] && s[i] < e[j] && s[j] < e[i]
                {
                    valid = false;
                }
            }
        }
        if valid {
            best = best.max(weight);
        }
    }
    best
}

fn check(s: &[i64], e: &[i64], w: &[i64]) -> (Vec<bool>, i128) {
    let expected = brute_force(s, e, w);
    assert_eq!(suffix_optimum(s, e, w), expected);
    let mask = solve(s, e, w).unwrap();
    assert_eq!(verify(s, e, w, &mask), expected);
    assert_eq!(mask, solve(s, e, w).unwrap());
    for (name, run) in CANDIDATES {
        let result = run(s, e, w).unwrap();
        assert_eq!(verify(s, e, w, &result.mask), expected, "{name}");
        assert_eq!(result.mask, run(s, e, w).unwrap().mask, "{name}");
    }
    (mask, expected)
}

#[test]
fn empty_and_single_rows() {
    assert_eq!(check(&[], &[], &[]), (vec![], 0));
    assert_eq!(check(&[1], &[5], &[10]), (vec![true], 10));
    assert_eq!(check(&[1], &[5], &[-10]), (vec![false], 0));
    assert_eq!(check(&[1], &[5], &[0]), (vec![false], 0));
}

#[test]
fn negative_disjoint_touching_and_conflict() {
    assert_eq!(check(&[0, 2], &[1, 3], &[-1, -2]), (vec![false; 2], 0));
    assert_eq!(check(&[0, 2], &[1, 3], &[10, 20]), (vec![true; 2], 30));
    assert_eq!(check(&[0, 1], &[1, 2], &[10, 20]), (vec![true; 2], 30));
    assert_eq!(check(&[0, 2], &[5, 4], &[10, 20]), (vec![false, true], 20));
}

#[test]
fn greedy_counterexamples() {
    // Largest-weight-first chooses 15; the optimum combines three tens.
    assert_eq!(
        check(&[0, 0, 4, 7], &[10, 4, 7, 10], &[15, 10, 10, 10]),
        (vec![false, true, true, true], 30)
    );
    // Earliest-finish chooses weight 1 instead of the overlapping weight 20.
    assert_eq!(check(&[0, 0], &[1, 4], &[1, 20]), (vec![false, true], 20));
}

#[test]
fn predecessor_chain_and_original_order() {
    let s = [1, 2, 4, 6, 5, 7];
    let e = [3, 5, 6, 7, 8, 9];
    let w = [5, 6, 5, 4, 11, 2];
    assert_eq!(
        check(&s, &e, &w),
        (vec![false, true, false, false, true, false], 17)
    );
    let order = [4, 0, 5, 2, 1, 3];
    let s = order.map(|i| s[i]);
    let e = order.map(|i| e[i]);
    let w = order.map(|i| w[i]);
    assert_eq!(
        check(&s, &e, &w),
        (vec![true, false, false, false, true, false], 17)
    );
}

#[test]
fn duplicates_and_empty_intervals() {
    let (mask, objective) = check(&[1; 4], &[5; 4], &[10; 4]);
    assert_eq!(objective, 10);
    assert_eq!(mask.iter().filter(|&&v| v).count(), 1);
    assert_eq!(
        check(&[2; 4], &[2; 4], &[3, -4, 0, 5]),
        (vec![true, false, false, true], 8)
    );
    // Empty intervals inside an ordinary interval remain compatible with it.
    assert_eq!(
        check(
            &[0, 2, 2, 2, 2, 5],
            &[5, 2, 2, 2, 2, 8],
            &[10, 3, -4, 0, 5, -9]
        ),
        (vec![true, true, false, false, true, false], 18)
    );
}

#[test]
fn integer_boundaries_and_generic_endpoints() {
    assert_eq!(
        solve(&[i64::MIN, 0], &[0, i64::MAX], &[u64::MAX; 2]),
        Ok(vec![true; 2])
    );
    assert_eq!(
        solve(&[0, 1, 2], &[1, 2, 3], &[i64::MAX, i64::MIN, i64::MAX]),
        Ok(vec![true, false, true])
    );
    assert_eq!(
        solve(&['a', 'b'], &['b', 'c'], &[1u8, 2]),
        Ok(vec![true; 2])
    );
    assert_eq!(
        solve(&[u64::MAX - 1, u64::MAX], &[u64::MAX; 2], &[1, 2]),
        Ok(vec![true; 2])
    );
    let tied = solve(&[0, 0], &[1, 1], &[i128::MAX; 2]).unwrap();
    assert_eq!(tied.iter().filter(|&&selected| selected).count(), 1);
    for (s, e) in [([0, 1], [1, 2]), ([0, 0], [0, 0]), ([0, 0], [1, 0])] {
        assert_eq!(
            solve(&s, &e, &[i128::MAX, 1]),
            Err(IntervalError::WeightOverflow)
        );
    }
}

#[test]
fn validation_and_error_messages() {
    assert_eq!(
        solve(&[1], &[], &[0]),
        Err(IntervalError::LengthMismatch {
            starts_len: 1,
            ends_len: 0
        })
    );
    for w in [vec![], vec![1, 2]] {
        assert_eq!(
            solve(&[1], &[2], &w),
            Err(IntervalError::WeightLengthMismatch {
                intervals_len: 1,
                weights_len: w.len(),
            })
        );
    }
    assert_eq!(
        solve(&[9, 0, 5, 3], &[10, 0, 4, 2], &[1, 0, -1, 2]),
        Err(IntervalError::InvalidInterval { index: 2 })
    );
    for (_, run) in CANDIDATES {
        assert!(matches!(
            run(&[1], &[], &[1]),
            Err(IntervalError::LengthMismatch { .. })
        ));
        assert!(matches!(
            run(&[1], &[2], &[]),
            Err(IntervalError::WeightLengthMismatch { .. })
        ));
        assert!(matches!(
            run(&[9, 0, 5], &[10, 0, 4], &[1, 0, -1]),
            Err(IntervalError::InvalidInterval { index: 2 })
        ));
    }
    assert!(IntervalError::WeightOverflow.to_string().contains("i128"));
    assert!(
        IntervalError::WeightLengthMismatch {
            intervals_len: 1,
            weights_len: 2
        }
        .to_string()
        .contains("1 intervals, 2 weights")
    );
}

fn instances() -> impl Strategy<Value = Vec<(i64, i64, i64)>> {
    prop::collection::vec((-4i64..=4, 0i64..=6, -10i64..=20), 0..=12).prop_map(|rows| {
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
    fn feasible_exact_shape_and_deterministic(rows in instances()) {
        let (s, e, w) = columns(&rows);
        check(&s, &e, &w);
    }

    #[test]
    fn translation_and_permutation(rows in instances(), offset in -100i64..=100, seed in 1u64..=u64::MAX) {
        let (s, e, w) = columns(&rows);
        let (mask, optimum) = check(&s, &e, &w);
        let shifted_s: Vec<_> = s.iter().map(|x| x + offset).collect();
        let shifted_e: Vec<_> = e.iter().map(|x| x + offset).collect();
        prop_assert_eq!(check(&shifted_s, &shifted_e, &w), (mask, optimum));
        let mut rows = rows;
        let mut state = seed;
        shuffle(&mut rows, &mut state);
        let (s, e, w) = columns(&rows);
        prop_assert_eq!(check(&s, &e, &w).1, optimum);
    }

    #[test]
    fn adding_negative_and_positive_empty(rows in instances(), x in -5i64..=5, length in 0i64..=6, weight in 1i64..=20) {
        let (mut s, mut e, mut w) = columns(&rows);
        let (_, optimum) = check(&s, &e, &w);
        s.push(x); e.push(x + length); w.push(-weight);
        let (mask, objective) = check(&s, &e, &w);
        prop_assert_eq!(objective, optimum);
        prop_assert!(!mask[mask.len() - 1]);
        e[s.len() - 1] = x; w[s.len() - 1] = weight;
        prop_assert_eq!(check(&s, &e, &w).1, optimum + i128::from(weight));
    }

    #[test]
    fn all_negative_and_positive_scaling(rows in instances(), factor in 1i64..=8) {
        let (s, e, w) = columns(&rows);
        let (_, optimum) = check(&s, &e, &w);
        let scaled: Vec<_> = w.iter().map(|w| w * factor).collect();
        prop_assert_eq!(check(&s, &e, &scaled).1, optimum * i128::from(factor));
        let negative: Vec<_> = w.iter().map(|w| -w.abs() - 1).collect();
        prop_assert_eq!(check(&s, &e, &negative), (vec![false; s.len()], 0));
    }
}
