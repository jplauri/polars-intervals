use intervals_core::{DiscreteEndpoint, IntervalError, minimum_stabbing_points};
use proptest::prelude::*;

#[path = "../benches/support/stabbing.rs"]
#[allow(dead_code)] // Phase timings are read by the release benchmark, not correctness tests.
mod candidates;
#[path = "support/stabbing.rs"]
mod oracle;

fn solve(rows: &[(i64, i64)]) -> Vec<i64> {
    let (s, e): (Vec<_>, Vec<_>) = rows.iter().copied().unzip();
    minimum_stabbing_points(&s, &e).unwrap()
}

fn check(rows: &[(i64, i64)], expected: &[i64]) {
    assert_eq!(solve(rows), expected);
    let (s, e): (Vec<_>, Vec<_>) = rows.iter().copied().unzip();
    for method in ["A", "B", "C"] {
        assert_eq!(candidates::run(&s, &e, method).unwrap().points, expected);
    }
    assert_eq!(oracle::packing(&s, &e), expected.len());
}

#[test]
fn empty_input() {
    check(&[], &[]);
}
#[test]
fn single_interval_uses_predecessor() {
    check(&[(1, 5)], &[4]);
}
#[test]
fn half_open_touching_boundary() {
    check(&[(0, 2), (2, 4)], &[1, 3]);
}
#[test]
fn touching_chain() {
    check(&[(0, 2), (2, 4), (4, 6), (6, 8)], &[1, 3, 5, 7]);
}
#[test]
fn common_intersection() {
    check(&[(0, 10), (2, 9), (4, 8), (5, 7)], &[6]);
}
#[test]
fn identical_duplicates() {
    check(&[(1, 5); 3], &[4]);
}
#[test]
fn deeply_nested() {
    check(&(0..100).map(|i| (i, 200 - i)).collect::<Vec<_>>(), &[100]);
}
#[test]
fn nested_then_separated() {
    check(&[(0, 10), (1, 3), (5, 9), (6, 8)], &[2, 7]);
}
#[test]
fn disjoint_intervals() {
    check(
        &(0..100).map(|i| (3 * i, 3 * i + 2)).collect::<Vec<_>>(),
        &(0..100).map(|i| 3 * i + 1).collect::<Vec<_>>(),
    );
}
#[test]
fn greedy_choice_regression() {
    check(&[(0, 10), (1, 2), (2, 3)], &[1, 2]);
}
#[test]
fn equal_ends_are_geometrically_deterministic() {
    check(&[(4, 5), (1, 5), (2, 5), (5, 8)], &[4, 7]);
}
#[test]
fn signed_extremes() {
    check(
        &[(i64::MIN, i64::MIN + 1), (i64::MAX - 1, i64::MAX)],
        &[i64::MIN, i64::MAX - 1],
    );
}
#[test]
fn unsigned_extremes() {
    assert_eq!(
        minimum_stabbing_points(&[u64::MAX - 1, 0], &[u64::MAX, 1]),
        Ok(vec![0, u64::MAX - 1])
    );
}
#[test]
fn predecessor_is_checked_for_every_integer_primitive() {
    macro_rules! check { ($($t:ty),*) => {$(
        assert_eq!(<$t>::MIN.predecessor(), None);
        assert_eq!(minimum_stabbing_points(&[<$t>::MIN], &[<$t>::MIN+1]), Ok(vec![<$t>::MIN]));
        assert_eq!(minimum_stabbing_points(&[<$t>::MAX-1], &[<$t>::MAX]), Ok(vec![<$t>::MAX-1]));
    )*}; }
    check!(
        i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
    );
}
#[test]
fn empty_interval_reports_original_index() {
    for (s, e, index) in [
        (vec![3], vec![3], 0),
        (vec![3, 0, 6], vec![3, 2, 9], 0),
        (vec![6, 3, 0], vec![9, 3, 2], 1),
        (vec![6, 0, 3], vec![9, 2, 3], 2),
    ] {
        let error = IntervalError::EmptyInterval { index };
        assert_eq!(minimum_stabbing_points(&s, &e), Err(error));
        assert_eq!(
            error.to_string(),
            format!("cannot stab empty interval at index {index}")
        );
        for method in ["A", "B", "C"] {
            assert_eq!(candidates::run(&s, &e, method).err(), Some(error));
        }
    }
}
#[test]
fn invalid_interval_and_lengths() {
    assert_eq!(
        minimum_stabbing_points(&[4, 3], &[5, 2]),
        Err(IntervalError::InvalidInterval { index: 1 })
    );
    for (s, e) in [(vec![1], vec![]), (vec![], vec![1])] {
        assert_eq!(
            minimum_stabbing_points(&s, &e),
            Err(IntervalError::LengthMismatch {
                starts_len: s.len(),
                ends_len: e.len()
            })
        );
    }
}

fn interval() -> impl Strategy<Value = (i64, i64)> {
    (-5i64..7).prop_flat_map(|s| (Just(s), s + 1..=7))
}
fn collections() -> impl Strategy<Value = Vec<(i64, i64)>> {
    prop::collection::vec(interval(), 0..=8)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn exact_against_all_coordinate_subsets_and_packing(rows in collections()) {
        let points = solve(&rows);
        prop_assert_eq!(Some(points.len()), oracle::brute_stabbing(&rows));
        prop_assert_eq!(points.len(), oracle::brute_packing(&rows));
        prop_assert!(rows.iter().all(|&(s,e)| points.iter().any(|&p| s <= p && p < e)));
        prop_assert!(points.windows(2).all(|w| w[0] < w[1]));
        prop_assert!(points.iter().all(|&p| rows.iter().any(|r| p == r.1-1)));
        prop_assert_eq!(solve(&rows), points.clone());
        check(&rows, &points);
    }
    #[test]
    fn permutation_translation_and_duplicates(rows in collections(), keys in prop::collection::vec(any::<u64>(), 8), d in -100i64..=100) {
        let expected = solve(&rows);
        let mut permuted: Vec<_> = rows.iter().copied().zip(keys).collect();
        permuted.sort_unstable_by_key(|r| r.1);
        prop_assert_eq!(solve(&permuted.iter().map(|r| r.0).collect::<Vec<_>>()), expected.clone());
        prop_assert_eq!(solve(&rows.iter().map(|&(s,e)| (s+d,e+d)).collect::<Vec<_>>()), expected.iter().map(|p| p+d).collect::<Vec<_>>());
        for &row in &rows {
            let mut duplicated = rows.clone(); duplicated.push(row);
            prop_assert_eq!(solve(&duplicated), expected.clone());
        }
    }
    #[test]
    fn adding_constraint_cannot_improve(rows in collections(), extra in interval()) {
        let old = solve(&rows).len();
        let mut added = rows; added.push(extra);
        prop_assert!(solve(&added).len() >= old);
    }
    #[test]
    fn disjoint_union_is_additive(a in collections(), b in collections()) {
        let mut expected = solve(&a); expected.extend(solve(&b).iter().map(|p| p+20));
        let mut union = a; union.extend(b.iter().map(|&(s,e)| (s+20,e+20)));
        prop_assert_eq!(solve(&union), expected);
    }
    #[test]
    fn guaranteed_common_point(p in -100i64..=100, widths in prop::collection::vec((0i64..10, 1i64..10), 1..=20)) {
        let rows: Vec<_> = widths.iter().map(|&(l,r)| (p-l,p+r)).collect();
        prop_assert_eq!(solve(&rows).len(), 1);
    }
    #[test]
    fn guaranteed_disjoint(widths in prop::collection::vec(1i64..10, 0..=30)) {
        let rows: Vec<_> = widths.iter().enumerate().map(|(i,&w)| (i as i64*10, i as i64*10+w)).collect();
        prop_assert_eq!(solve(&rows).len(), rows.len());
    }
    #[test]
    fn empty_constraint_is_infeasible(rows in collections(), x in -5i64..=7, pos in any::<usize>()) {
        let mut rows = rows;
        let index = pos % (rows.len()+1);
        rows.insert(index, (x,x));
        let (s,e): (Vec<_>, Vec<_>) = rows.iter().copied().unzip();
        prop_assert_eq!(minimum_stabbing_points(&s,&e), Err(IntervalError::EmptyInterval { index }));
        prop_assert_eq!(oracle::brute_stabbing(&rows), None);
    }
}
