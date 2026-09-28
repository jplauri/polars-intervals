#[allow(dead_code)]
#[path = "../benches/support/containment.rs"]
mod reference;

use intervals_core::{IntervalError, containment_counts};
use proptest::prelude::*;
use reference::naive;

fn check<T: Ord + Copy + std::fmt::Debug>(s: &[T], e: &[T], expected: &[usize]) {
    assert_eq!(naive(s, e), expected);
    assert_eq!(containment_counts(s, e).unwrap(), expected);
}

#[test]
fn empty_input() {
    check::<i64>(&[], &[], &[]);
}
#[test]
fn single_interval() {
    check(&[1], &[5], &[0]);
}
#[test]
fn disjoint_intervals() {
    check(&[0, 2, 4], &[1, 3, 5], &[0, 0, 0]);
}
#[test]
fn simple_nesting() {
    check(&[0, 2, 3], &[10, 8, 5], &[2, 1, 0]);
}
#[test]
fn deep_nesting_chain() {
    let s: Vec<_> = (0..1000).collect();
    let e: Vec<_> = s.iter().map(|i| 2000 - i).collect();
    check(&s, &e, &(0..1000).rev().collect::<Vec<_>>());
}
#[test]
fn partial_crossing_is_not_containment() {
    check(&[0, 3], &[5, 8], &[0, 0]);
}
#[test]
fn same_start_different_ends() {
    check(&[1, 1, 1], &[10, 8, 5], &[2, 1, 0]);
}
#[test]
fn same_end_different_starts() {
    check(&[0, 2, 5], &[10, 10, 10], &[2, 1, 0]);
}
#[test]
fn identical_duplicates() {
    check(&[1; 7], &[5; 7], &[6; 7]);
}
#[test]
fn duplicates_plus_nested_rows() {
    check(&[0, 2, 0, 3, 2], &[10, 8, 10, 5, 8], &[4, 2, 4, 0, 2]);
}
#[test]
fn full_equal_start_group_is_visible_in_every_order() {
    for (ends, counts) in [
        ([5, 5, 10], [1, 1, 2]),
        ([10, 5, 5], [2, 1, 1]),
        ([5, 10, 5], [1, 2, 1]),
    ] {
        check(&[1, 1, 1], &ends, &counts);
    }
}
#[test]
fn empty_at_interior_and_both_boundaries_is_contained() {
    check(&[0, 0, 5, 10], &[10, 0, 5, 10], &[3, 0, 0, 0]);
    check(&[0, 5], &[5, 5], &[1, 0]);
}
#[test]
fn identical_empty_intervals_contain_each_other() {
    check(&[3, 3], &[3, 3], &[1, 1]);
}
#[test]
fn empty_outer_only_contains_empties_at_its_coordinate() {
    check(
        &[3, 2, 4, 3, 3, 2],
        &[3, 2, 4, 3, 4, 3],
        &[1, 0, 0, 1, 3, 3],
    );
}
#[test]
fn boundary_touching_nonempty_intervals() {
    check(&[0, 5], &[5, 10], &[0, 0]);
}
#[test]
fn extreme_signed_endpoints_need_no_arithmetic() {
    check(
        &[i64::MIN, i64::MIN, 0, i64::MAX],
        &[i64::MAX, 0, i64::MAX, i64::MAX],
        &[3, 0, 1, 0],
    );
}
#[test]
fn extreme_unsigned_endpoints_need_no_arithmetic() {
    check(
        &[0, u64::MAX - 2, u64::MAX - 1, u64::MAX],
        &[u64::MAX; 4],
        &[3, 2, 1, 0],
    );
}
#[test]
fn original_row_order() {
    check(&[2, 0, 3, 1], &[8, 10, 7, 9], &[1, 3, 0, 2]);
}
#[test]
fn comparison_only_nonnumeric_endpoints() {
    check(&['a', 'c', 'd'], &['z', 'f', 'd'], &[2, 1, 0]);
}
#[test]
fn invalid_interval_reports_first_original_index() {
    assert_eq!(
        containment_counts(&[0, 3, 4], &[0, 2, 1]),
        Err(IntervalError::InvalidInterval { index: 1 })
    );
    assert_eq!(
        containment_counts(&[2], &[1]),
        Err(IntervalError::InvalidInterval { index: 0 })
    );
}
#[test]
fn length_mismatch_precedes_interval_validation() {
    for (s, e) in [
        (&[1][..], &[][..]),
        (&[][..], &[1][..]),
        (&[2, 3][..], &[1][..]),
    ] {
        assert_eq!(
            containment_counts(s, e),
            Err(IntervalError::LengthMismatch {
                starts_len: s.len(),
                ends_len: e.len()
            })
        );
    }
}

fn interval() -> impl Strategy<Value = (i32, i32)> {
    (-8i32..=8, -8i32..=8).prop_map(|(a, b)| (a.min(b), a.max(b)))
}
fn collection() -> impl Strategy<Value = (Vec<i32>, Vec<i32>)> {
    prop::collection::vec(interval(), 0..=30).prop_map(|v| v.into_iter().unzip())
}
fn counts(s: &[i32], e: &[i32]) -> Vec<usize> {
    containment_counts(s, e).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn exact_agreement_with_naive((s, e) in collection()) {
        check(&s, &e, &naive(&s, &e));
    }

    #[test]
    fn output_length((s, e) in collection()) {
        prop_assert_eq!(counts(&s, &e).len(), s.len());
    }

    #[test]
    fn count_bounds((s, e) in collection()) {
        for count in counts(&s, &e) {
            prop_assert!(count < s.len());
        }
    }

    #[test]
    fn single_row_excludes_self((s, e) in interval()) {
        prop_assert_eq!(counts(&[s], &[e]), [0]);
    }

    #[test]
    fn permutation_equivariance(
        (s, e) in collection(),
        keys in prop::collection::vec(any::<u64>(), 30),
    ) {
        let mut order: Vec<_> = (0..s.len()).collect();
        order.sort_unstable_by_key(|&i| (keys[i], i));
        let permuted_starts: Vec<_> = order.iter().map(|&i| s[i]).collect();
        let permuted_ends: Vec<_> = order.iter().map(|&i| e[i]).collect();
        let original = counts(&s, &e);
        let permuted = counts(&permuted_starts, &permuted_ends);
        for (i, count) in order.into_iter().zip(permuted) {
            prop_assert_eq!(count, original[i]);
        }
    }

    #[test]
    fn translation_invariance((s, e) in collection(), offset in -100i32..=100) {
        let starts: Vec<_> = s.iter().map(|x| x + offset).collect();
        let ends: Vec<_> = e.iter().map(|x| x + offset).collect();
        prop_assert_eq!(counts(&starts, &ends), counts(&s, &e));
    }

    #[test]
    fn positive_scaling_invariance((s, e) in collection(), factor in 1i32..=10) {
        let starts: Vec<_> = s.iter().map(|x| x * factor).collect();
        let ends: Vec<_> = e.iter().map(|x| x * factor).collect();
        prop_assert_eq!(counts(&starts, &ends), counts(&s, &e));
    }

    #[test]
    fn reflection_invariance((s, e) in collection()) {
        let starts: Vec<_> = e.iter().map(|x| -x).collect();
        let ends: Vec<_> = s.iter().map(|x| -x).collect();
        prop_assert_eq!(counts(&starts, &ends), counts(&s, &e));
    }

    #[test]
    fn appending_has_exact_predicate_delta((mut s, mut e) in collection(), (a, b) in interval()) {
        let before = counts(&s, &e);
        s.push(a);
        e.push(b);
        let after = counts(&s, &e);
        for i in 0..before.len() {
            let contains_new = s[i] <= a && b <= e[i];
            prop_assert_eq!(after[i], before[i] + usize::from(contains_new));
        }
    }

    #[test]
    fn duplicate_insertion((mut s, mut e) in collection(), pick in any::<usize>()) {
        if !s.is_empty() {
            let i = pick % s.len();
            let n = s.len();
            let before = counts(&s, &e);
            s.push(s[i]);
            e.push(e[i]);
            let after = counts(&s, &e);
            prop_assert_eq!(after[i], before[i] + 1);
            prop_assert_eq!(after[n], after[i]);
            for j in 0..n {
                let contains_duplicate = s[j] <= s[i] && e[i] <= e[j];
                prop_assert_eq!(after[j], before[j] + usize::from(contains_duplicate));
            }
        }
    }

    #[test]
    fn expanding_cannot_reduce_own_count(
        (mut s, mut e) in collection(),
        pick in any::<usize>(),
        a in 0i32..=10,
        b in 0i32..=10,
    ) {
        if !s.is_empty() {
            let i = pick % s.len();
            let before = counts(&s, &e)[i];
            s[i] -= a;
            e[i] += b;
            prop_assert!(counts(&s, &e)[i] >= before);
        }
    }

    #[test]
    fn shrinking_cannot_increase_own_count(
        (mut s, mut e) in collection(),
        pick in any::<usize>(),
        a in 0i32..=16,
        b in 0i32..=16,
    ) {
        if !s.is_empty() {
            let i = pick % s.len();
            let before = counts(&s, &e)[i];
            s[i] += a.min(e[i] - s[i]);
            e[i] -= b.min(e[i] - s[i]);
            prop_assert!(counts(&s, &e)[i] <= before);
        }
    }

    #[test]
    fn deep_chain_structure(n in 0i32..=100, offset in -100i32..=100) {
        let s: Vec<_> = (0..n).map(|i| offset + i).collect();
        let e: Vec<_> = (0..n).map(|i| offset + 2 * n - i).collect();
        prop_assert_eq!(counts(&s, &e), (0..n as usize).rev().collect::<Vec<_>>());
    }

    #[test]
    fn identical_family_structure(n in 0usize..=100, (s, e) in interval()) {
        prop_assert_eq!(counts(&vec![s; n], &vec![e; n]), vec![n.saturating_sub(1); n]);
    }

    #[test]
    fn pairwise_crossing_antichain(n in 0i32..=100) {
        let s: Vec<_> = (0..n).collect();
        let e: Vec<_> = (0..n).map(|i| i + n + 1).collect();
        prop_assert_eq!(counts(&s, &e), vec![0; n as usize]);
    }

    #[test]
    fn dual_pair_total((s, e) in collection()) {
        let mut incoming = vec![0usize; s.len()];
        for i in 0..s.len() {
            for j in 0..s.len() {
                if i != j && s[i] <= s[j] && e[j] <= e[i] {
                    incoming[j] += 1;
                }
            }
        }
        prop_assert_eq!(counts(&s, &e).iter().sum::<usize>(), incoming.iter().sum::<usize>());
    }
}

#[test]
fn exhaustive_small_collections() {
    let rows: Vec<_> = (-1..=2)
        .flat_map(|a| (a..=2).map(move |b| (a, b)))
        .collect();
    for n in 0..=5 {
        for mut code in 0..rows.len().pow(n) {
            let mut s = Vec::new();
            let mut e = Vec::new();
            for _ in 0..n {
                let (a, b) = rows[code % rows.len()];
                code /= rows.len();
                s.push(a);
                e.push(b);
            }
            check(&s, &e, &naive(&s, &e));
        }
    }
}
