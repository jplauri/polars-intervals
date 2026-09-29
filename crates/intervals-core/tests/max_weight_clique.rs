use intervals_core::{IntervalError, assign_lanes, max_clique, max_weight_clique};
use proptest::prelude::*;

// Independent graph oracle: enumerate every subset, including the empty set,
// and inspect EVERY selected pair. Empty singletons pass; an empty vertex in
// any pair fails. This deliberately does not call sweep/reconstruction helpers.
fn brute_force(s: &[i64], e: &[i64], w: &[i64]) -> (i128, usize) {
    let mut best = 0;
    let mut ways = 0;
    for bits in 0usize..1 << s.len() {
        let mut feasible = true;
        let mut objective = 0i128;
        for i in 0..s.len() {
            if bits & (1 << i) == 0 {
                continue;
            }
            objective += i128::from(w[i]);
            for j in 0..i {
                if bits & (1 << j) != 0
                    && !(s[i] < e[i] && s[j] < e[j] && s[i] < e[j] && s[j] < e[i])
                {
                    feasible = false;
                }
            }
        }
        if feasible {
            if objective > best {
                best = objective;
                ways = 1;
            } else if objective == best {
                ways += 1;
            }
        }
    }
    (best, ways)
}

// A separately structured O(n²) oracle scores input start coordinates directly,
// without sorting or maintaining active state. It also implements the tie rule
// independently of production.
fn coordinate_reference(s: &[i64], e: &[i64], w: &[i64]) -> (Vec<bool>, i128) {
    let mut coordinate = None;
    let mut best = 0i128;
    for &t in s {
        let score = (0..s.len())
            .filter(|&i| w[i] > 0 && s[i] <= t && t < e[i])
            .map(|i| i128::from(w[i]))
            .sum();
        if score > best || (score == best && score > 0 && coordinate.is_none_or(|old| t < old)) {
            best = score;
            coordinate = Some(t);
        }
    }
    let mut singleton = None;
    for i in 0..s.len() {
        if s[i] == e[i] && i128::from(w[i]) > best {
            singleton = Some(i);
            best = i128::from(w[i]);
        }
    }
    let mask = (0..s.len())
        .map(|i| {
            if let Some(j) = singleton {
                i == j
            } else {
                w[i] > 0 && coordinate.is_some_and(|t| s[i] <= t && t < e[i])
            }
        })
        .collect();
    (mask, best)
}

fn verify(s: &[i64], e: &[i64], w: &[i64], mask: &[bool]) -> i128 {
    assert_eq!(mask.len(), s.len());
    let mut objective = 0;
    for i in 0..s.len() {
        if mask[i] {
            assert!(w[i] > 0, "nonpositive row {i} selected");
            objective += i128::from(w[i]);
            for j in 0..i {
                if mask[j] {
                    assert!(s[i] < e[i] && s[j] < e[j] && s[i] < e[j] && s[j] < e[i]);
                }
            }
        }
    }
    objective
}

fn check(s: &[i64], e: &[i64], w: &[i64]) -> (Vec<bool>, i128) {
    let (expected, optimum) = coordinate_reference(s, e, w);
    assert_eq!(brute_force(s, e, w).0, optimum);
    let actual = max_weight_clique(s, e, w).unwrap();
    assert_eq!(verify(s, e, w, &actual), optimum);
    assert_eq!(actual, expected);
    assert_eq!(actual, max_weight_clique(s, e, w).unwrap());
    (actual, optimum)
}

fn check_units(s: &[i64], e: &[i64]) -> Vec<bool> {
    let units = vec![1; s.len()];
    let (expected, optimum) = check(s, e, &units);
    assert_eq!(max_clique(s, e).unwrap(), expected);
    let mut lanes = assign_lanes(s, e).unwrap();
    lanes.sort_unstable();
    lanes.dedup();
    assert_eq!(lanes.len() as i128, optimum);
    expected
}

#[test]
fn no_rows_and_singletons_in_both_modes() {
    assert_eq!(check(&[], &[], &[]), (vec![], 0));
    assert_eq!(check_units(&[], &[]), Vec::<bool>::new());
    for end in [1, 5] {
        assert_eq!(check(&[1], &[end], &[7]), (vec![true], 7));
        assert_eq!(check(&[1], &[end], &[0]), (vec![false], 0));
        assert_eq!(check(&[1], &[end], &[-7]), (vec![false], 0));
        assert_eq!(check_units(&[1], &[end]), vec![true]);
    }
}

#[test]
fn empty_vertices_are_isolated_even_at_duplicate_coordinates() {
    assert_eq!(
        check(&[2, 2, 0, 8, 2], &[2, 2, 0, 8, 2], &[3, 7, 7, -1, 0]),
        (vec![false, true, false, false, false], 7)
    );
    assert_eq!(check(&[2; 3], &[2; 3], &[-1, 0, -3]), (vec![false; 3], 0));
    assert_eq!(
        check_units(&[8, 2, 2], &[8, 2, 2]),
        vec![true, false, false]
    );
}

#[test]
fn disjoint_and_touching_intervals_do_not_accumulate() {
    assert_eq!(check(&[0, 2], &[2, 4], &[5, 7]), (vec![false, true], 7));
    assert_eq!(
        check(&[0, 2, 1], &[2, 4, 3], &[5, 7, 4]),
        (vec![false, true, true], 11)
    );
    assert_eq!(check(&[0, 9], &[1, 10], &[5, 7]), (vec![false, true], 7));
}

#[test]
fn overlap_chain_and_star_are_not_cliques() {
    assert_eq!(
        check(&[0, 1, 2], &[2, 3, 4], &[4, 5, 6]),
        (vec![false, true, true], 11)
    );
    assert_eq!(
        check(&[0, 0, 2, 4, 6], &[10, 1, 3, 5, 7], &[3, 2, 7, 4, 6]),
        (vec![true, false, true, false, false], 10)
    );
}

#[test]
fn full_clique_nested_and_duplicate_vertices_accumulate() {
    assert_eq!(
        check(&[0, 1, 2], &[7, 6, 5], &[2, 3, 5]),
        (vec![true; 3], 10)
    );
    assert_eq!(
        check(&[0, 1, 2], &[4, 5, 6], &[2, 3, 5]),
        (vec![true; 3], 10)
    );
    assert_eq!(
        check(&[1; 4], &[5; 4], &[2, 3, -7, 0]),
        (vec![true, true, false, false], 5)
    );
    assert_eq!(check_units(&[1; 4], &[5; 4]), vec![true; 4]);
}

#[test]
fn weight_objective_differs_from_cardinality_and_has_no_secondary_objective() {
    let (s, e) = ([0, 1, 2, 10], [5, 5, 5, 11]);
    assert_eq!(
        check(&s, &e, &[1, 1, 1, 9]),
        (vec![false, false, false, true], 9)
    );
    assert_eq!(check_units(&s, &e), vec![true, true, true, false]);
    // An earlier singleton beats a tied later two-row clique under the tie rule.
    assert_eq!(
        check(&[0, 2, 2], &[1, 4, 4], &[4, 2, 2]),
        (vec![true, false, false], 4)
    );
}

#[test]
fn harmful_active_vertices_are_omitted() {
    assert_eq!(
        check(&[0, 1, 2], &[5, 4, 3], &[10, -100, 9]),
        (vec![true, false, true], 19)
    );
}

#[test]
fn common_intersection_detection_ignores_discarded_rows_and_cannot_restart() {
    assert_eq!(
        check(&[9, 0, 1, 7], &[10, 5, 4, 7], &[-50, 2, 3, 5]),
        (vec![false, true, true, false], 5)
    );
    // The wide third row cannot restore the empty intersection of rows 0/1.
    assert_eq!(
        check(&[0, 3, -5], &[1, 4, 10], &[2, 3, 4]),
        (vec![false, true, true], 7)
    );
    assert_eq!(
        check_units(&[0, 3, -5], &[1, 4, 10]),
        vec![true, false, true]
    );
}

#[test]
fn empty_singleton_beats_loses_and_ties_nonempty_clique() {
    let (s, e) = ([3, 1, 2, 3], [3, 5, 4, 3]);
    assert_eq!(
        check(&s, &e, &[8, 2, 3, 8]),
        (vec![true, false, false, false], 8)
    );
    assert_eq!(
        check(&s, &e, &[4, 2, 3, 4]),
        (vec![false, true, true, false], 5)
    );
    assert_eq!(
        check(&s, &e, &[5, 2, 3, 5]),
        (vec![false, true, true, false], 5)
    );
    assert_eq!(check_units(&[0, 0], &[0, 1]), vec![false, true]);
}

#[test]
fn earliest_completed_coordinate_wins_ties() {
    // Row order does not choose the winning nonempty component.
    assert_eq!(check(&[8, 0], &[9, 1], &[7, 7]), (vec![false, true], 7));
    assert_eq!(check_units(&[8, 0], &[9, 1]), vec![false, true]);
    // Arrival/departure batches must be fully processed at a coordinate.
    assert_eq!(
        check(
            &[0, 0, 2, 2, 1, 2],
            &[2, 2, 4, 4, 3, 2],
            &[2, 3, 4, 5, 6, 3]
        ),
        (vec![false, false, true, true, true, false], 15)
    );
    assert_eq!(
        check(&[0, 0, 1], &[3, 2, 2], &[2, 3, 5]),
        (vec![true; 3], 10)
    );
}

#[test]
fn disconnected_components_restore_original_row_order() {
    let s = [10, 0, 11, 1, 12, 2];
    let e = [15, 5, 15, 5, 15, 5];
    let w = [5, 2, 6, 3, 7, 4];
    assert_eq!(
        check(&s, &e, &w),
        (vec![true, false, true, false, true, false], 18)
    );
    let order = [3, 4, 0, 5, 2, 1];
    assert_eq!(
        check(
            &order.map(|i| s[i]),
            &order.map(|i| e[i]),
            &order.map(|i| w[i])
        ),
        (vec![false, true, true, false, true, false], 18)
    );
}

#[test]
fn sorted_reverse_partial_and_shuffled_inputs_preserve_unique_solution() {
    let rows = [
        (0, 10, 2),
        (1, 9, 3),
        (2, 8, 4),
        (3, 7, 5),
        (4, 6, 6),
        (11, 12, 1),
    ];
    for order in [
        [0, 1, 2, 3, 4, 5],
        [5, 4, 3, 2, 1, 0],
        [0, 1, 4, 3, 2, 5],
        [3, 0, 5, 2, 4, 1],
    ] {
        let (s, e, w) = columns(&order.map(|i| rows[i]));
        assert_eq!(check(&s, &e, &w), (order.map(|i| i != 5).to_vec(), 20));
    }
}

#[test]
fn signed_unsigned_narrow_and_nonnumeric_endpoints() {
    assert_eq!(
        max_weight_clique(
            &[i64::MIN, 0, i64::MAX],
            &[0, i64::MAX, i64::MAX],
            &[2, 3, 1]
        ),
        Ok(vec![false, true, false])
    );
    assert_eq!(
        max_clique(&[u64::MAX - 2, u64::MAX - 1], &[u64::MAX; 2]),
        Ok(vec![true; 2])
    );
    assert_eq!(
        max_weight_clique(&['a', 'b', 'c'], &['d', 'e', 'c'], &[2, 3, 4]),
        Ok(vec![true, true, false])
    );
    assert_eq!(
        max_clique(&['a', 'b', 'c'], &['d', 'e', 'c']),
        Ok(vec![true, true, false])
    );
    macro_rules! endpoints { ($($t:ty),*) => {$({
        let (s, e) = ([<$t>::MIN, <$t>::MAX - 1], [<$t>::MAX, <$t>::MAX]);
        assert_eq!(max_clique(&s, &e), Ok(vec![true; 2]));
        assert_eq!(max_weight_clique(&s, &e, &[1u8, 2]), Ok(vec![true; 2]));
    })*}; }
    endpoints!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);
}

#[test]
fn exact_i128_arithmetic_only_overflows_for_a_clique() {
    assert_eq!(
        max_weight_clique(&[0, 0], &[1, 1], &[i128::MAX, 1]),
        Err(IntervalError::WeightOverflow)
    );
    for (s, e) in [([0, 2], [1, 3]), ([0, 1], [1, 2]), ([0, 0], [0, 0])] {
        assert_eq!(
            max_weight_clique(&s, &e, &[i128::MAX; 2]),
            Ok(vec![true, false])
        );
    }
    assert_eq!(
        max_weight_clique(&[0, 0, 1], &[2, 2, 1], &[i128::MAX, i128::MIN, i128::MAX]),
        Ok(vec![true, false, false])
    );
    assert_eq!(
        max_weight_clique(&[0, 0], &[1, 1], &[u64::MAX; 2]),
        Ok(vec![true; 2])
    );
    assert_eq!(
        check(&[0, 2], &[1, 3], &[1 << 53, (1 << 53) + 1]),
        (vec![false, true], (1 << 53) + 1)
    );
    macro_rules! weights { ($($t:ty),*) => {$({
        assert_eq!(max_weight_clique(&[0, 0], &[2, 2], &[1 as $t, 2 as $t]), Ok(vec![true; 2]));
    })*}; }
    weights!(i8, i16, i32, i64, i128, u8, u16, u32, u64);
}

#[test]
fn validates_all_rows_before_fast_paths_and_optimization() {
    for weights in [[0i128, 0, 0], [i128::MAX, 1, -1], [1, 1, 0]] {
        for (s, e) in [([0, 0, 5], [1, 1, 4]), ([0, 0, 5], [0, 0, 4])] {
            assert_eq!(
                max_weight_clique(&s, &e, &weights),
                Err(IntervalError::InvalidInterval { index: 2 })
            );
            assert_eq!(
                max_clique(&s, &e),
                Err(IntervalError::InvalidInterval { index: 2 })
            );
        }
    }
    assert_eq!(
        max_weight_clique(&[9, 0, 5, 3], &[10, 0, 4, 2], &[1, 0, -1, 1]),
        Err(IntervalError::InvalidInterval { index: 2 })
    );
    for (s, e) in [(&[1][..], &[][..]), (&[][..], &[1][..])] {
        let error = IntervalError::LengthMismatch([("starts", s.len()), ("ends", e.len())]);
        assert_eq!(max_weight_clique(s, e, &[1]), Err(error));
        assert_eq!(max_clique(s, e), Err(error));
    }
    for w in [vec![], vec![1, 2]] {
        assert_eq!(
            max_weight_clique(&[0], &[1], &w),
            Err(IntervalError::LengthMismatch([
                ("intervals", 1),
                ("weights", w.len())
            ]))
        );
    }
    assert!(
        IntervalError::InvalidInterval { index: 7 }
            .to_string()
            .contains("index 7")
    );
    assert!(IntervalError::WeightOverflow.to_string().contains("i128"));
    assert!(
        IntervalError::LengthMismatch([("intervals", 1), ("weights", 2)])
            .to_string()
            .contains("1 intervals, 2 weights")
    );
}

#[test]
fn large_nested_then_disjoint_input_reconstructs_once_after_many_improvements() {
    let n = 100_000i64;
    let mut s: Vec<_> = (0..n).collect();
    let mut e: Vec<_> = (0..n).map(|i| 2 * n - i).collect();
    // Force the general sweep so repeated improvements exercise reconstruction
    // even after adding the common-intersection fast path.
    s.push(3 * n);
    e.push(3 * n + 1);
    let w = vec![1i64; s.len()];
    for mask in [
        max_clique(&s, &e).unwrap(),
        max_weight_clique(&s, &e, &w).unwrap(),
    ] {
        // Linear common-intersection/membership verification, no pairwise scan.
        assert_eq!(mask.len(), s.len());
        let largest_start = s
            .iter()
            .zip(&mask)
            .filter(|(_, selected)| **selected)
            .map(|(&x, _)| x)
            .max()
            .unwrap();
        let smallest_end = e
            .iter()
            .zip(&mask)
            .filter(|(_, selected)| **selected)
            .map(|(&x, _)| x)
            .min()
            .unwrap();
        assert!(largest_start < smallest_end);
        assert!(mask[..n as usize].iter().all(|&selected| selected));
        assert!(!mask[n as usize]);
    }
}

#[test]
fn exhaustive_dense_tiny_instances_include_zero_rows() {
    let choices: Vec<_> = (0..=2)
        .flat_map(|s| (s..=2).flat_map(move |e| [-1, 0, 2].map(|w| (s, e, w))))
        .collect();
    for n in 0..=3 {
        for mut code in 0..choices.len().pow(n) {
            let rows: Vec<_> = (0..n)
                .map(|_| {
                    let row = choices[code % choices.len()];
                    code /= choices.len();
                    row
                })
                .collect();
            let (s, e, w) = columns(&rows);
            check(&s, &e, &w);
        }
    }
}

fn columns(rows: &[(i64, i64, i64)]) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

fn instances(max: usize) -> impl Strategy<Value = Vec<(i64, i64, i64)>> {
    // Dense ties, duplicates, empties and nesting all arise constructively.
    prop::collection::vec((-4i64..=4, -4i64..=4, -8i64..=12), 0..=max).prop_map(|rows| {
        rows.into_iter()
            .map(|(a, b, w)| (a.min(b), a.max(b), w))
            .collect()
    })
}

// ProptestConfig::default honors PROPTEST_CASES; standard source-parallel
// regression persistence preserves minimized failures alongside this suite.
proptest! {
    #[test]
    fn weighted_and_units_are_optimal_feasible_and_deterministic(rows in instances(10)) {
        let (s, e, w) = columns(&rows);
        check(&s, &e, &w);
        check_units(&s, &e);
    }

    #[test]
    fn larger_instances_match_independent_coordinate_reference(rows in instances(100)) {
        let (s, e, w) = columns(&rows);
        let (expected, optimum) = coordinate_reference(&s, &e, &w);
        let mask = max_weight_clique(&s, &e, &w).unwrap();
        prop_assert_eq!(verify(&s, &e, &w, &mask), optimum);
        prop_assert_eq!(&mask, &expected);
    }

    #[test]
    fn translation_and_strictly_increasing_relabeling_preserve_masks(rows in instances(50), delta in -1000i64..=1000) {
        let (s, e, w) = columns(&rows);
        let mask = max_weight_clique(&s, &e, &w).unwrap();
        let units = max_clique(&s, &e).unwrap();
        for transform in [0, 1] {
            // x^3 is strictly increasing and changes unequal coordinate gaps.
            let relabel = |x: &i64| if transform == 0 { x + delta } else { x * x * x + delta };
            let ts: Vec<_> = s.iter().map(relabel).collect();
            let te: Vec<_> = e.iter().map(relabel).collect();
            prop_assert_eq!(max_weight_clique(&ts, &te, &w).unwrap(), mask.clone());
            prop_assert_eq!(max_clique(&ts, &te).unwrap(), units.clone());
        }
    }

    #[test]
    fn permutations_preserve_objective_and_unique_masks(rows in instances(10), keys in prop::collection::vec(any::<u64>(), 10)) {
        let (s, e, w) = columns(&rows);
        let (mask, optimum) = check(&s, &e, &w);
        let mut order: Vec<_> = (0..rows.len()).collect();
        order.sort_unstable_by_key(|&i| (keys[i], i));
        let shuffled: Vec<_> = order.iter().map(|&i| rows[i]).collect();
        let (ps, pe, pw) = columns(&shuffled);
        let selected = max_weight_clique(&ps, &pe, &pw).unwrap();
        prop_assert_eq!(verify(&ps, &pe, &pw, &selected), optimum);
        let mut restored = vec![false; rows.len()];
        for (j, &i) in order.iter().enumerate() { restored[i] = selected[j]; }
        prop_assert_eq!(verify(&s, &e, &w, &restored), optimum);
        // Earliest-coordinate ties are also row-permutation invariant; only
        // equal empty singletons intentionally depend on original row order.
        if brute_force(&s, &e, &w).1 == 1 || mask.iter().enumerate().any(|(i, &yes)| yes && s[i] < e[i]) {
            prop_assert_eq!(restored, mask);
        }
        let ones = vec![1; s.len()];
        let unit_mask = max_clique(&s, &e).unwrap();
        let (unit_optimum, unit_ways) = brute_force(&s, &e, &ones);
        let unit_selected = max_clique(&ps, &pe).unwrap();
        prop_assert_eq!(verify(&ps, &pe, &ones, &unit_selected), unit_optimum);
        let mut unit_restored = vec![false; rows.len()];
        for (j, &i) in order.iter().enumerate() { unit_restored[i] = unit_selected[j]; }
        prop_assert_eq!(verify(&s, &e, &ones, &unit_restored), unit_optimum);
        if unit_ways == 1 || unit_mask.iter().enumerate().any(|(i, &yes)| yes && s[i] < e[i]) {
            prop_assert_eq!(unit_restored, unit_mask);
        }
    }

    #[test]
    fn positive_scaling_and_nonpositive_addition(rows in instances(40), factor in 1i64..=8, a in -5i64..=5, b in -5i64..=5, extra in -10i64..=0) {
        let (mut s, mut e, mut w) = columns(&rows);
        let (mask, optimum) = coordinate_reference(&s, &e, &w);
        let scaled: Vec<_> = w.iter().map(|x| x * factor).collect();
        let selected = max_weight_clique(&s, &e, &scaled).unwrap();
        prop_assert_eq!(verify(&s, &e, &scaled, &selected), optimum * i128::from(factor));
        prop_assert_eq!(selected, mask.clone());
        s.push(a.min(b)); e.push(a.max(b)); w.push(extra);
        let selected = max_weight_clique(&s, &e, &w).unwrap();
        prop_assert_eq!(verify(&s, &e, &w, &selected), optimum);
        prop_assert_eq!(&selected[..mask.len()], mask.as_slice());
        prop_assert!(!selected[mask.len()]);
    }

    #[test]
    fn positive_empty_addition_takes_maximum_not_sum(rows in instances(40), x in -5i64..=5, q in 1i64..=30) {
        let (mut s, mut e, mut w) = columns(&rows);
        let optimum = coordinate_reference(&s, &e, &w).1;
        s.push(x); e.push(x); w.push(q);
        let selected = max_weight_clique(&s, &e, &w).unwrap();
        prop_assert_eq!(verify(&s, &e, &w, &selected), optimum.max(i128::from(q)));
    }

    #[test]
    fn disjoint_union_takes_maximum_not_sum(left in instances(30), right in instances(30)) {
        let (mut s, mut e, mut w) = columns(&left);
        let optimum_left = coordinate_reference(&s, &e, &w).1;
        let (rs, re, rw) = columns(&right);
        let optimum_right = coordinate_reference(&rs, &re, &rw).1;
        // Original bounds [-4,4] make translation by 20 strictly disjoint.
        s.extend(rs.iter().map(|x| x + 20)); e.extend(re.iter().map(|x| x + 20)); w.extend(rw);
        let selected = max_weight_clique(&s, &e, &w).unwrap();
        prop_assert_eq!(verify(&s, &e, &w, &selected), optimum_left.max(optimum_right));
    }

    #[test]
    fn duplicating_nonempty_rows_doubles_objective(rows in instances(5)) {
        let rows: Vec<_> = rows.into_iter().filter(|r| r.0 < r.1).collect();
        let (mut s, mut e, mut w) = columns(&rows);
        let optimum = brute_force(&s, &e, &w).0;
        s.extend_from_within(..); e.extend_from_within(..); w.extend_from_within(..);
        prop_assert_eq!(check(&s, &e, &w).1, 2 * optimum);
    }

    #[test]
    fn extreme_integer_endpoints_need_only_order(rows in prop::collection::vec((any::<i64>(), any::<i64>(), -4i64..=8), 0..=10)) {
        let rows: Vec<_> = rows.into_iter().map(|(a, b, w)| (a.min(b), a.max(b), w)).collect();
        let (s, e, w) = columns(&rows);
        check(&s, &e, &w);
        check_units(&s, &e);
    }
}
