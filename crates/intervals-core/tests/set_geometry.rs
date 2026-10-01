#[path = "../benches/support/set_geometry_candidates.rs"]
mod candidates;
#[path = "support/set_geometry.rs"]
mod oracle;

use intervals_core::{
    IntervalError, IntervalSetError, intersect_intervals, interval_gaps, merge_intervals,
    subtract_intervals,
};
use proptest::prelude::*;

fn unzip<T: Copy>(rows: &[(T, T)]) -> (Vec<T>, Vec<T>) {
    rows.iter().copied().unzip()
}

fn run<T: Ord + Copy>(left: &[(T, T)], right: &[(T, T)], intersection: bool) -> Vec<(T, T)> {
    let (ls, le) = unzip(left);
    let (rs, re) = unzip(right);
    if intersection {
        intersect_intervals(&ls, &le, &rs, &re).unwrap()
    } else {
        subtract_intervals(&ls, &le, &rs, &re).unwrap()
    }
}

fn union<T: Ord + Copy>(rows: &[(T, T)]) -> Vec<(T, T)> {
    let (s, e) = unzip(rows);
    merge_intervals(&s, &e).unwrap()
}

fn canonical<T: Ord + Copy>(output: &[(T, T)]) {
    assert!(output.iter().all(|&(s, e)| s < e));
    assert!(output.windows(2).all(|pair| pair[0].1 < pair[1].0));
}

fn check<T: Ord + Copy + std::fmt::Debug>(
    left: &[(T, T)],
    right: &[(T, T)],
    difference: &[(T, T)],
    intersection: &[(T, T)],
) {
    let (ls, le) = unzip(left);
    let (rs, re) = unzip(right);
    for (intersect, expected) in [(false, difference), (true, intersection)] {
        assert_eq!(oracle::cells(&ls, &le, &rs, &re, intersect), expected);
        for method in candidates::METHODS {
            let actual = if intersect {
                candidates::intersect(method, &ls, &le, &rs, &re)
            } else {
                candidates::subtract(method, &ls, &le, &rs, &re)
            }
            .unwrap();
            canonical(&actual);
            assert_eq!(actual, expected, "{method}, intersection={intersect}");
        }
    }
}

#[test]
fn empty_operands_and_empty_rows_contribute_no_coverage() {
    check::<i32>(&[], &[], &[], &[]);
    check(&[(0, 0), (5, 5)], &[], &[], &[]);
    check(&[], &[(0, 3)], &[], &[]);
    check(&[(0, 3)], &[], &[(0, 3)], &[]);
    check(&[(0, 3)], &[(1, 1), (3, 3)], &[(0, 3)], &[]);
    check(&[(0, 0), (0, 3), (1, 1)], &[(0, 0), (3, 3)], &[(0, 3)], &[]);
}

#[test]
fn singleton_geometry_handles_containment_partial_overlap_disjoint_and_touching() {
    type Ranges = [(i32, i32)];
    let cases: &[(&Ranges, &Ranges, &Ranges)] = &[
        (&[(0, 10)], &[], &[(0, 10)]),
        (&[(-1, 11)], &[], &[(0, 10)]),
        (&[(2, 8)], &[(0, 2), (8, 10)], &[(2, 8)]),
        (&[(-1, 3)], &[(3, 10)], &[(0, 3)]),
        (&[(7, 11)], &[(0, 7)], &[(7, 10)]),
        (&[(-3, -1)], &[(0, 10)], &[]),
        (&[(11, 13)], &[(0, 10)], &[]),
        (&[(-2, 0)], &[(0, 10)], &[]),
        (&[(10, 12)], &[(0, 10)], &[]),
    ];
    for (right, difference, intersection) in cases {
        check(&[(0, 10)], right, difference, intersection);
    }
    check(&[(0, 2)], &[(2, 4)], &[(0, 2)], &[]);
}

#[test]
fn worked_example_discards_original_row_boundaries() {
    check(
        &[(0, 5), (4, 10), (12, 15)],
        &[(2, 3), (6, 8), (10, 13)],
        &[(0, 2), (3, 6), (8, 10), (13, 15)],
        &[(2, 3), (6, 8), (12, 13)],
    );
}

#[test]
fn exclusions_split_a_left_run_and_preserve_its_final_tail() {
    check(
        &[(0, 10)],
        &[(2, 4), (6, 8)],
        &[(0, 2), (4, 6), (8, 10)],
        &[(2, 4), (6, 8)],
    );
}

#[test]
fn right_run_is_reused_across_left_runs() {
    check(
        &[(0, 3), (5, 8)],
        &[(1, 7)],
        &[(0, 1), (7, 8)],
        &[(1, 3), (5, 7)],
    );
    check(&[(0, 3), (5, 8)], &[(-1, 9)], &[], &[(0, 3), (5, 8)]);
}

#[test]
fn nesting_tracks_the_furthest_end_on_both_sides() {
    check(
        &[(0, 100), (1, 2), (2, 2), (3, 4)],
        &[(50, 60)],
        &[(0, 50), (60, 100)],
        &[(50, 60)],
    );
    check(
        &[(0, 100)],
        &[(20, 80), (21, 22), (23, 24), (70, 70)],
        &[(0, 20), (80, 100)],
        &[(20, 80)],
    );
}

#[test]
fn redundant_exclusions_form_one_boolean_set() {
    check(
        &[(0, 10)],
        &[(2, 8), (3, 5), (8, 9), (2, 8)],
        &[(0, 2), (9, 10)],
        &[(2, 9)],
    );
    check(&[(0, 10), (0, 10)], &[(0, 10)], &[], &[(0, 10)]);
}

#[test]
fn output_coalesces_across_source_boundaries_but_never_real_gaps() {
    check(&[(0, 2), (2, 4)], &[(1, 3)], &[(0, 1), (3, 4)], &[(1, 3)]);
    check(&[(0, 2), (3, 4)], &[(1, 4)], &[(0, 1)], &[(1, 2), (3, 4)]);
    check(
        &[(0, 1), (1, 2), (2, 3)],
        &[(0, 1), (1, 2), (2, 3)],
        &[],
        &[(0, 3)],
    );
}

#[test]
fn tied_endpoints_empties_and_irrelevant_outer_rows_do_not_split_outputs() {
    check(
        &[(0, 5), (0, 3), (2, 5), (4, 4)],
        &[(-9, -2), (2, 4), (2, 3), (3, 4), (9, 10)],
        &[(0, 2), (4, 5)],
        &[(2, 4)],
    );
}

#[test]
fn extremes_and_nonnumeric_endpoints_need_no_coordinate_arithmetic() {
    check(
        &[
            (i64::MIN, i64::MAX),
            (i64::MIN, i64::MIN),
            (i64::MAX, i64::MAX),
        ],
        &[(0, 1)],
        &[(i64::MIN, 0), (1, i64::MAX)],
        &[(0, 1)],
    );
    let large = (1u64 << 53) + 1;
    check(
        &[(large, u64::MAX)],
        &[(large + 1, u64::MAX - 1)],
        &[(large, large + 1), (u64::MAX - 1, u64::MAX)],
        &[(large + 1, u64::MAX - 1)],
    );
    check(
        &[(i8::MIN, i8::MAX)],
        &[(-1, 1)],
        &[(i8::MIN, -1), (1, i8::MAX)],
        &[(-1, 1)],
    );
    check(
        &[('a', 'z')],
        &[('c', 'f'), ('f', 'g')],
        &[('a', 'c'), ('g', 'z')],
        &[('c', 'g')],
    );
}

#[test]
fn many_holes_and_duplicates_have_linear_output() {
    let right: Vec<_> = (0..10_000).map(|i| (4 * i + 1, 4 * i + 3)).collect();
    let difference: Vec<_> = std::iter::once((0, 1))
        .chain((1..10_000).map(|i| (4 * i - 1, 4 * i + 1)))
        .chain(std::iter::once((39_999, 40_000)))
        .collect();
    let (rs, re) = unzip(&right);
    for method in candidates::METHODS {
        assert_eq!(
            candidates::subtract(method, &[0], &[40_000], &rs, &re).unwrap(),
            difference
        );
        assert_eq!(
            candidates::intersect(method, &[0], &[40_000], &rs, &re).unwrap(),
            right
        );
        assert!(
            candidates::subtract(method, &vec![0; 20_000], &vec![10; 20_000], &[0], &[10])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            candidates::intersect(
                method,
                &vec![0; 20_000],
                &vec![10; 20_000],
                &vec![0; 10_000],
                &vec![10; 10_000]
            )
            .unwrap(),
            [(0, 10)]
        );
    }
}

#[test]
fn every_candidate_reports_original_side_and_row_before_fast_paths() {
    use IntervalSetError::{Left, Right};
    let reversed = IntervalError::InvalidInterval { index: 2 };
    for method in candidates::METHODS {
        for operation in [candidates::subtract::<i32>, candidates::intersect::<i32>] {
            assert_eq!(
                operation(method, &[0, 10, 8], &[100, 10, 7], &[], &[]),
                Err(Left(reversed))
            );
            assert_eq!(
                operation(method, &[], &[], &[0, 10, 8], &[100, 10, 7]),
                Err(Right(reversed))
            );
            assert_eq!(
                operation(method, &[1], &[0], &[2], &[1]),
                Err(Left(IntervalError::InvalidInterval { index: 0 }))
            );
            // Full cover, disjoint envelopes, and empty sets must not prune bad rows.
            assert_eq!(
                operation(method, &[0], &[1], &[-100, 100, 8], &[100, 100, 7]),
                Err(Right(reversed))
            );
            for (s, e) in [(&[0][..], &[][..]), (&[][..], &[0][..])] {
                let error = IntervalError::LengthMismatch([("starts", s.len()), ("ends", e.len())]);
                assert_eq!(operation(method, s, e, &[], &[]), Err(Left(error)));
                assert_eq!(operation(method, &[], &[], s, e), Err(Right(error)));
            }
        }
    }
    let error = Right(IntervalError::InvalidInterval { index: 2 });
    assert!(error.to_string().contains("right"));
    assert!(error.to_string().contains('2'));
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn subtraction_is_neither_commutative_nor_associative() {
    let a = [(0, 4)];
    let b = [(1, 3)];
    let c = [(2, 4)];
    assert_ne!(run(&a, &b, false), run(&b, &a, false));
    assert_ne!(
        run(&run(&a, &b, false), &c, false),
        run(&a, &run(&b, &c, false), false)
    );
}

fn rows() -> impl Strategy<Value = Vec<(i32, i32)>> {
    prop::collection::vec((-16i32..=16, -16i32..=16), 0..25).prop_map(|rows| {
        rows.into_iter()
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect()
    })
}

fn covered(rows: &[(i32, i32)], tick: i32) -> bool {
    rows.iter().any(|&(s, e)| s <= tick && tick < e)
}

proptest! {
    #[test]
    fn all_candidates_match_two_independent_oracles(left in rows(), right in rows()) {
        let (ls, le) = unzip(&left);
        let (rs, re) = unzip(&right);
        let difference = oracle::cells(&ls, &le, &rs, &re, false);
        let intersection = oracle::cells(&ls, &le, &rs, &re, true);
        prop_assert_eq!(&difference, &oracle::bitmap(&ls, &le, &rs, &re, false));
        prop_assert_eq!(&intersection, &oracle::bitmap(&ls, &le, &rs, &re, true));
        check(&left, &right, &difference, &intersection);
    }

    #[test]
    fn independent_permutations_duplicates_and_empties_preserve_sets(left in rows(), right in rows(), pivot in any::<usize>()) {
        let mut a = left.clone();
        let mut b = right.clone();
        a.reverse();
        if !b.is_empty() { let shift = pivot % b.len(); b.rotate_left(shift); }
        for intersection in [false, true] {
            let expected = run(&left, &right, intersection);
            prop_assert_eq!(run(&a, &b, intersection), expected.clone());
            let mut duplicates = a.clone();
            duplicates.extend_from_slice(&a);
            duplicates.extend([(-16, -16), (0, 0), (16, 16)]);
            let mut right_duplicates = b.clone();
            right_duplicates.extend_from_slice(&b);
            right_duplicates.extend([(0, 0), (16, 16)]);
            prop_assert_eq!(run(&duplicates, &right_duplicates, intersection), expected);
        }
    }

    #[test]
    fn premerging_either_operand_and_remerging_output_preserves_results(left in rows(), right in rows()) {
        let a = union(&left);
        let b = union(&right);
        for intersection in [false, true] {
            let result = run(&left, &right, intersection);
            prop_assert_eq!(&run(&a, &right, intersection), &result);
            prop_assert_eq!(&run(&left, &b, intersection), &result);
            prop_assert_eq!(&run(&a, &b, intersection), &result);
            prop_assert_eq!(&union(&result), &result);
        }
    }

    #[test]
    fn identity_laws_and_intersection_commutativity(left in rows(), right in rows()) {
        prop_assert_eq!(run(&left, &right, true), run(&right, &left, true));
        prop_assert_eq!(run(&left, &left, true), union(&left));
        prop_assert!(run(&left, &[], true).is_empty());
        prop_assert!(run(&left, &left, false).is_empty());
        prop_assert_eq!(run(&left, &[], false), union(&left));
        prop_assert!(run(&[], &right, false).is_empty());
    }

    #[test]
    fn difference_and_intersection_partition_left_exactly(left in rows(), right in rows()) {
        let difference = run(&left, &right, false);
        let intersection = run(&left, &right, true);
        let combined: Vec<_> = difference.iter().chain(&intersection).copied().collect();
        prop_assert_eq!(union(&combined), union(&left));
        for tick in -16..16 {
            let d = covered(&difference, tick);
            let i = covered(&intersection, tick);
            prop_assert!(!d || (covered(&left, tick) && !covered(&right, tick)));
            prop_assert!(!i || (covered(&left, tick) && covered(&right, tick)));
            prop_assert!(!(d && i));
        }
        let area = |rows: &[(i32, i32)]| rows.iter().map(|&(s, e)| i64::from(e) - i64::from(s)).sum::<i64>();
        prop_assert_eq!(area(&difference) + area(&intersection), area(&union(&left)));
    }

    #[test]
    fn repeated_difference_and_three_collection_laws(a in rows(), b in rows(), c in rows()) {
        let difference = run(&a, &b, false);
        prop_assert_eq!(run(&difference, &b, false), difference.clone());
        prop_assert_eq!(run(&run(&a, &b, true), &c, true), run(&a, &run(&b, &c, true), true));
        let bc: Vec<_> = b.iter().chain(&c).copied().collect();
        prop_assert_eq!(run(&difference, &c, false), run(&a, &union(&bc), false));
    }

    #[test]
    fn adding_intervals_has_set_monotonicity_not_fragment_count_monotonicity(a in rows(), b in rows(), extra in rows()) {
        let aa: Vec<_> = a.iter().chain(&extra).copied().collect();
        let bb: Vec<_> = b.iter().chain(&extra).copied().collect();
        for intersection in [false, true] {
            let before = run(&a, &b, intersection);
            let more_left = run(&aa, &b, intersection);
            let more_right = run(&a, &bb, intersection);
            for tick in -16..16 {
                prop_assert!(!covered(&before, tick) || covered(&more_left, tick));
                if intersection {
                    prop_assert!(!covered(&before, tick) || covered(&more_right, tick));
                } else {
                    prop_assert!(!covered(&more_right, tick) || covered(&before, tick));
                }
            }
        }
    }

    #[test]
    fn translations_and_strictly_increasing_relabelings_preserve_geometry(a in rows(), b in rows(), delta in -1000i32..1000) {
        for relabel in [|x: i32| x, |x: i32| x * x * x] {
            let map = |rows: &[(i32, i32)]| rows.iter().map(|&(s, e)| (relabel(s) + delta, relabel(e) + delta)).collect::<Vec<_>>();
            for intersection in [false, true] {
                prop_assert_eq!(run(&map(&a), &map(&b), intersection), map(&run(&a, &b, intersection)));
            }
        }
    }

    #[test]
    fn splitting_rows_at_touching_boundaries_preserves_both_sets(a in rows(), b in rows()) {
        let split = |rows: &[(i32, i32)]| rows.iter().flat_map(|&(s, e)| {
            let middle = (s + e).div_euclid(2);
            [(s, middle), (middle, e)]
        }).collect::<Vec<_>>();
        for intersection in [false, true] {
            let expected = run(&a, &b, intersection);
            prop_assert_eq!(run(&split(&a), &b, intersection), expected.clone());
            prop_assert_eq!(run(&a, &split(&b), intersection), expected);
        }
    }

    #[test]
    fn singleton_domain_difference_matches_existing_gap_operation(b in rows(), bounds in (-16i32..=16, -16i32..=16)) {
        let (l, r) = (bounds.0.min(bounds.1), bounds.0.max(bounds.1));
        let (s, e) = unzip(&b);
        prop_assert_eq!(run(&[(l, r)], &b, false), interval_gaps(&s, &e, l, r).unwrap());
    }

    #[test]
    fn larger_wide_inputs_have_only_original_boundaries_and_linear_output(
        a in prop::collection::vec((any::<i64>(), any::<i64>()), 0..300),
        b in prop::collection::vec((any::<i64>(), any::<i64>()), 0..300),
    ) {
        let a: Vec<_> = a.into_iter().map(|(s, e)| (s.min(e), s.max(e))).collect();
        let b: Vec<_> = b.into_iter().map(|(s, e)| (s.min(e), s.max(e))).collect();
        let boundaries: std::collections::BTreeSet<_> = a.iter().chain(&b).flat_map(|&(s, e)| [s, e]).collect();
        let (ls, le) = unzip(&a);
        let (rs, re) = unzip(&b);
        for intersection in [false, true] {
            let expected = run(&a, &b, intersection);
            canonical(&expected);
            prop_assert!(expected.len() <= boundaries.len() / 2);
            for &(s, e) in &expected {
                prop_assert!(boundaries.contains(&s) && boundaries.contains(&e));
            }
            for method in candidates::METHODS {
                let actual = if intersection { candidates::intersect(method, &ls, &le, &rs, &re) } else { candidates::subtract(method, &ls, &le, &rs, &re) }.unwrap();
                prop_assert_eq!(&actual, &expected);
            }
            prop_assert_eq!(run(&a, &b, intersection), expected);
        }
    }
}
