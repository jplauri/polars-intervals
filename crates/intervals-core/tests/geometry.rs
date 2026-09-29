#[path = "../benches/support/geometry_candidates.rs"]
mod candidates;
#[path = "support/geometry.rs"]
mod oracle;

use candidates::{CLUSTER_METHODS, GAP_METHODS, MERGE_METHODS};
use intervals_core::{
    IntervalError, cluster_intervals, coverage_profile, interval_gaps, merge_intervals,
    validate_intervals,
};
use proptest::prelude::*;

fn rows() -> impl Strategy<Value = (Vec<i32>, Vec<i32>)> {
    prop::collection::vec((-12i32..=12, -12i32..=12), 0..25)
        .prop_map(|rows| rows.into_iter().map(|(a, b)| (a.min(b), a.max(b))).unzip())
}

fn bounds() -> impl Strategy<Value = (i32, i32)> {
    (-16i32..=16, -16i32..=16).prop_map(|(a, b)| (a.min(b), a.max(b)))
}

fn check<T: Ord + Copy + std::fmt::Debug>(s: &[T], e: &[T], domain: (T, T)) {
    for touching in [false, true] {
        let expected = oracle::graph(s, e, touching);
        for method in CLUSTER_METHODS {
            assert_eq!(
                candidates::cluster(method, s, e, touching).unwrap(),
                expected,
                "cluster {method}"
            );
        }
    }
    let expected = oracle::cells(s, e, None);
    for method in MERGE_METHODS {
        assert_eq!(
            candidates::merge(method, s, e).unwrap(),
            expected,
            "merge {method}"
        );
    }
    let expected = oracle::cells(s, e, Some(domain));
    for method in GAP_METHODS {
        assert_eq!(
            candidates::gaps(method, s, e, domain.0, domain.1).unwrap(),
            expected,
            "gaps {method}"
        );
    }
}

fn unzip<T: Copy>(rows: &[(T, T)]) -> (Vec<T>, Vec<T>) {
    rows.iter().copied().unzip()
}

fn canonical<T: Ord + Copy + std::fmt::Debug>(ranges: &[(T, T)]) {
    assert!(ranges.iter().all(|(s, e)| s < e));
    assert!(ranges.windows(2).all(|pair| pair[0].1 < pair[1].0));
}

fn covered<T: Ord + Copy>(ranges: &[(T, T)], x: T) -> bool {
    ranges.iter().any(|&(s, e)| s <= x && x < e)
}

#[test]
fn typed_empty_singletons_and_empty_only_collections() {
    for (s, e) in [
        (&[][..], &[][..]),
        (&[2][..], &[4][..]),
        (&[2][..], &[2][..]),
        (&[2, 2, 2][..], &[2, 2, 2][..]),
    ] {
        check::<i32>(s, e, (0, 6));
    }
    assert_eq!(
        cluster_intervals(&[2, 2, 2], &[2, 2, 2], true).unwrap(),
        [0, 1, 2]
    );
    assert_eq!(merge_intervals(&[2, 2, 2], &[2, 2, 2]).unwrap(), []);
    assert_eq!(interval_gaps::<i32>(&[], &[], 0, 6).unwrap(), [(0, 6)]);
}

#[test]
fn worked_example_has_connected_components_union_and_bounded_gaps() {
    let (s, e) = (&[1, 3, 5, 8, 12], &[4, 6, 8, 10, 15]);
    check(s, e, (0, 16));
    assert_eq!(cluster_intervals(s, e, false).unwrap(), [0, 0, 0, 1, 2]);
    assert_eq!(cluster_intervals(s, e, true).unwrap(), [0, 0, 0, 0, 1]);
    assert_eq!(merge_intervals(s, e).unwrap(), [(1, 10), (12, 15)]);
    assert_eq!(
        interval_gaps(s, e, 0, 16).unwrap(),
        [(0, 1), (10, 12), (15, 16)]
    );
}

#[test]
fn frontier_is_the_maximum_end_despite_nesting_and_interleaved_empties() {
    let (s, e) = (&[0, 0, 1, 2, 3, 100, 110], &[100, 0, 2, 2, 4, 100, 110]);
    check(s, e, (-1, 111));
    for touching in [false, true] {
        assert_eq!(
            cluster_intervals(s, e, touching).unwrap(),
            [0, 1, 0, 2, 0, 3, 4]
        );
    }
}

#[test]
fn touching_and_overlap_chains_need_no_common_intersection() {
    check(&[0, 2, 4, 10, 11, 12], &[2, 4, 6, 12, 13, 14], (-1, 15));
    assert_eq!(
        cluster_intervals(&[0, 2, 4], &[2, 4, 6], false).unwrap(),
        [0, 1, 2]
    );
    assert_eq!(
        cluster_intervals(&[0, 2, 4], &[2, 4, 6], true).unwrap(),
        [0, 0, 0]
    );
    assert_eq!(
        cluster_intervals(&[0, 1, 2], &[2, 3, 4], false).unwrap(),
        [0, 0, 0]
    );
}

#[test]
fn empty_points_never_bridge_touching_rows_or_split_gaps() {
    let (s, e) = (&[-1, 0, 1, 2, 2, 3, 4, 5], &[-1, 2, 1, 2, 4, 3, 4, 5]);
    check(s, e, (-2, 6));
    assert_eq!(
        cluster_intervals(s, e, false).unwrap(),
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(
        cluster_intervals(s, e, true).unwrap(),
        [0, 1, 2, 3, 1, 4, 5, 6]
    );
    assert_eq!(
        interval_gaps(&[0, 2, 4], &[0, 2, 4], -1, 5).unwrap(),
        [(-1, 5)]
    );
}

#[test]
fn repeated_starts_ends_duplicates_and_full_nesting() {
    check(
        &[0, 0, 1, 2, 3, 1, 0, 8],
        &[9, 8, 8, 8, 4, 8, 9, 9],
        (-1, 10),
    );
    check(&[8, 0, 8, 0], &[9, 1, 9, 1], (-1, 10));
}

#[test]
fn component_ids_follow_first_input_occurrence_not_coordinate_order() {
    check(&[10, 0, 1], &[12, 2, 3], (-1, 15));
    assert_eq!(
        cluster_intervals(&[10, 0, 1], &[12, 2, 3], false).unwrap(),
        [0, 1, 1]
    );
    assert_eq!(
        merge_intervals(&[10, 0, 1], &[12, 2, 3]).unwrap(),
        [(0, 3), (10, 12)]
    );
    check(&[8, 3, 3, 0, 8, 2], &[9, 4, 3, 2, 9, 3], (-1, 10));
}

#[test]
fn domain_empty_outside_spanning_clipped_and_exact_boundary_cases() {
    for domain in [(0, 0), (0, 10), (-20, -10), (20, 30), (2, 8)] {
        for (s, e) in [
            (&[-10, 10][..], &[0, 20][..]),
            (&[-10][..], &[20][..]),
            (&[-2, 7][..], &[3, 12][..]),
            (&[2, 5][..], &[4, 8][..]),
            (&[0, 10][..], &[0, 10][..]),
        ] {
            check(s, e, domain);
        }
    }
}

#[test]
fn every_candidate_validates_original_rows_before_clipping_or_empty_domain() {
    let (s, e) = (&[30, 0, 20], &[40, 1, 10]);
    let error = IntervalError::InvalidInterval { index: 2 };
    assert_eq!(validate_intervals(s, e), Err(error));
    for method in CLUSTER_METHODS {
        assert_eq!(candidates::cluster(method, s, e, false), Err(error));
    }
    for method in MERGE_METHODS {
        assert_eq!(candidates::merge(method, s, e), Err(error));
    }
    for method in GAP_METHODS {
        for (l, r) in [(0, 0), (-20, -10), (100, 200)] {
            assert_eq!(candidates::gaps(method, s, e, l, r), Err(error));
        }
    }
}

#[test]
fn every_candidate_rejects_mismatched_lengths_and_reversed_domains() {
    for (s, e) in [(&[0][..], &[][..]), (&[][..], &[1][..])] {
        let error = IntervalError::LengthMismatch([("starts", s.len()), ("ends", e.len())]);
        assert_eq!(validate_intervals(s, e), Err(error));
        for method in CLUSTER_METHODS {
            assert_eq!(candidates::cluster(method, s, e, true), Err(error));
        }
        for method in MERGE_METHODS {
            assert_eq!(candidates::merge(method, s, e), Err(error));
        }
        for method in GAP_METHODS {
            assert_eq!(candidates::gaps(method, s, e, 0, 0), Err(error));
        }
    }
    for method in GAP_METHODS {
        assert_eq!(
            candidates::gaps(method, &[0], &[0], 1, 0),
            Err(IntervalError::InvalidDomain)
        );
    }
}

#[test]
fn comparison_only_extremes_narrow_unsigned_and_character_endpoints() {
    check(
        &[i64::MIN, 0, i64::MAX],
        &[0, i64::MAX, i64::MAX],
        (i64::MIN, i64::MAX),
    );
    check(
        &[0, u64::MAX - 1, 1 << 53, (1 << 53) + 1],
        &[1, u64::MAX, (1 << 53) + 1, (1 << 53) + 2],
        (0, u64::MAX),
    );
    check(
        &[i8::MIN, 0, i8::MAX],
        &[0, i8::MAX, i8::MAX],
        (i8::MIN, i8::MAX),
    );
    check(&[0u8, 254, 255], &[1, 255, 255], (0, 255));
    check(&['z', 'a', 'b', 'd'], &['z', 'c', 'd', 'e'], ('a', 'z'));
}

#[test]
fn sorted_reversed_shuffled_partial_and_sorted_starts_unsorted_ends() {
    let rows = [
        (0, 100),
        (1, 2),
        (2, 80),
        (3, 4),
        (4, 70),
        (5, 5),
        (101, 102),
    ];
    for order in [
        [0, 1, 2, 3, 4, 5, 6],
        [6, 5, 4, 3, 2, 1, 0],
        [3, 0, 5, 1, 6, 4, 2],
        [1, 0, 3, 2, 5, 4, 6],
    ] {
        let (s, e): (Vec<_>, Vec<_>) = order.into_iter().map(|i| rows[i]).unzip();
        check(&s, &e, (-1, 103));
    }
}

#[test]
fn presorted_detection_ignores_arbitrarily_ordered_empties_and_clipped_out_rows() {
    check(&[8, 0, 99, 1, -99, 2], &[8, 2, 99, 3, -99, 4], (-1, 5));
    assert_eq!(
        cluster_intervals(&[8, 0, 99, 1, -99, 2], &[8, 2, 99, 3, -99, 4], false).unwrap(),
        [0, 1, 2, 1, 3, 1]
    );
    check(&[-100, 1, -300, 3, 200], &[-90, 2, -290, 4, 210], (0, 5));
}

#[test]
fn splitting_a_row_preserves_union_but_can_change_strict_components() {
    assert_eq!(
        merge_intervals(&[0], &[4]),
        merge_intervals(&[0, 2], &[2, 4])
    );
    assert_eq!(
        interval_gaps(&[0], &[4], -1, 5),
        interval_gaps(&[0, 2], &[2, 4], -1, 5)
    );
    assert_eq!(cluster_intervals(&[0, 2], &[2, 4], false).unwrap(), [0, 1]);
    assert_eq!(cluster_intervals(&[0, 2], &[2, 4], true).unwrap(), [0, 0]);
}

#[test]
fn adding_intervals_can_increase_or_decrease_component_and_gap_counts() {
    assert_eq!(merge_intervals(&[0, 4], &[2, 6]).unwrap().len(), 2);
    assert_eq!(merge_intervals(&[0, 4, 1], &[2, 6, 5]).unwrap().len(), 1);
    assert_eq!(merge_intervals(&[0, 4, 8], &[2, 6, 9]).unwrap().len(), 3);
    assert_eq!(interval_gaps(&[0, 4], &[2, 6], 0, 10).unwrap().len(), 2);
    assert_eq!(
        interval_gaps(&[0, 4, 7], &[2, 6, 8], 0, 10).unwrap().len(),
        3
    );
}

#[test]
fn large_structured_collections_have_known_answers_without_quadratic_oracles() {
    let n = 100_000i64;
    let starts: Vec<_> = (0..n).map(|i| 3 * i).collect();
    let ends: Vec<_> = starts.iter().map(|s| s + 1).collect();
    assert_eq!(
        cluster_intervals(&starts, &ends, false).unwrap(),
        (0..n as u32).collect::<Vec<_>>()
    );
    assert_eq!(merge_intervals(&starts, &ends).unwrap().len(), n as usize);
    assert_eq!(
        interval_gaps(&starts, &ends, 0, 3 * n).unwrap().len(),
        n as usize
    );
    let mut spanning_ends = ends;
    spanning_ends[0] = 3 * n;
    assert!(
        cluster_intervals(&starts, &spanning_ends, false)
            .unwrap()
            .iter()
            .all(|&id| id == 0)
    );
    assert_eq!(
        merge_intervals(&starts, &spanning_ends).unwrap(),
        [(0, 3 * n)]
    );
    assert!(
        interval_gaps(&starts, &spanning_ends, 0, 3 * n)
            .unwrap()
            .is_empty()
    );
}

proptest! {
    #[test]
    fn every_retained_candidate_matches_graph_cells_and_bitmap(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        check(&s, &e, domain);
        prop_assert_eq!(oracle::cells(&s, &e, None), oracle::bitmap(&s, &e, None));
        prop_assert_eq!(
            oracle::cells(&s, &e, Some(domain)),
            oracle::bitmap(&s, &e, Some(domain))
        );
        prop_assert!(validate_intervals(&s, &e).is_ok());
    }

    #[test]
    fn wide_comparison_only_candidates_match_independent_oracles(
        pairs in prop::collection::vec((0usize..8, 0usize..8), 0..20),
        a in 0usize..8,
        b in 0usize..8,
    ) {
        let points = [
            i64::MIN,
            i64::MIN + 1,
            -(1 << 53),
            -1,
            0,
            1 << 53,
            i64::MAX - 1,
            i64::MAX,
        ];
        let (s, e): (Vec<_>, Vec<_>) = pairs
            .into_iter()
            .map(|(a, b)| (points[a.min(b)], points[a.max(b)]))
            .unzip();
        check(&s, &e, (points[a.min(b)], points[a.max(b)]));
    }

    #[test]
    fn canonical_ids_empties_refinement_and_determinism(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        let strict = cluster_intervals(&s, &e, false).unwrap();
        let touching = cluster_intervals(&s, &e, true).unwrap();
        for mode in [false, true] {
            let ids = cluster_intervals(&s, &e, mode).unwrap();
            prop_assert_eq!(&ids, &cluster_intervals(&s, &e, mode).unwrap());
            prop_assert_eq!(ids.len(), s.len());
            let mut seen = std::collections::BTreeSet::new();
            for &id in &ids {
                if seen.insert(id) {
                    prop_assert_eq!(id as usize, seen.len() - 1);
                }
            }
            for i in 0..s.len() {
                for j in 0..s.len() {
                    if s[i] == e[i] && i != j {
                        prop_assert_ne!(ids[i], ids[j]);
                    }
                    if strict[i] == strict[j] {
                        prop_assert_eq!(touching[i], touching[j]);
                    }
                }
            }
        }
        let merged = merge_intervals(&s, &e).unwrap();
        let gaps = interval_gaps(&s, &e, domain.0, domain.1).unwrap();
        canonical(&merged);
        canonical(&gaps);
        prop_assert_eq!(&merged, &merge_intervals(&s, &e).unwrap());
        prop_assert_eq!(&gaps, &interval_gaps(&s, &e, domain.0, domain.1).unwrap());
        for &(l, r) in &gaps {
            prop_assert!(domain.0 <= l && r <= domain.1);
        }
    }

    #[test]
    fn arbitrary_permutation_preserves_sets_and_mapped_partitions(
        triples in prop::collection::vec((-12i32..=12, -12i32..=12, any::<u32>()), 0..30),
        domain in bounds(),
    ) {
        let (s, e): (Vec<_>, Vec<_>) = triples
            .iter()
            .map(|&(a, b, _)| (a.min(b), a.max(b)))
            .unzip();
        let mut order: Vec<_> = (0..s.len()).collect();
        order.sort_unstable_by_key(|&i| (triples[i].2, i));
        let (ps, pe): (Vec<_>, Vec<_>) = order.iter().map(|&i| (s[i], e[i])).unzip();
        prop_assert_eq!(merge_intervals(&s, &e), merge_intervals(&ps, &pe));
        prop_assert_eq!(
            interval_gaps(&s, &e, domain.0, domain.1),
            interval_gaps(&ps, &pe, domain.0, domain.1)
        );
        for mode in [false, true] {
            let original = cluster_intervals(&s, &e, mode).unwrap();
            let permuted = cluster_intervals(&ps, &pe, mode).unwrap();
            for i in 0..s.len() {
                for j in 0..s.len() {
                    prop_assert_eq!(
                        original[order[i]] == original[order[j]],
                        permuted[i] == permuted[j]
                    );
                }
            }
        }
    }

    #[test]
    fn translation_and_strictly_increasing_relabeling_preserve_topology(
        (s, e) in rows(),
        domain in bounds(),
        delta in -1000i32..1000,
    ) {
        for cubic in [false, true] {
            let map = |x: i32| if cubic { x * x * x + delta } else { x + delta };
            let ss: Vec<_> = s.iter().copied().map(map).collect();
            let ee: Vec<_> = e.iter().copied().map(map).collect();
            for mode in [false, true] {
                prop_assert_eq!(
                    cluster_intervals(&s, &e, mode),
                    cluster_intervals(&ss, &ee, mode)
                );
            }
            let expected: Vec<_> = merge_intervals(&s, &e)
                .unwrap()
                .into_iter()
                .map(|(a, b)| (map(a), map(b)))
                .collect();
            prop_assert_eq!(merge_intervals(&ss, &ee).unwrap(), expected);
            let expected: Vec<_> = interval_gaps(&s, &e, domain.0, domain.1)
                .unwrap()
                .into_iter()
                .map(|(a, b)| (map(a), map(b)))
                .collect();
            prop_assert_eq!(
                interval_gaps(&ss, &ee, map(domain.0), map(domain.1)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn adding_empties_and_nonempty_duplicates_preserves_original_partition(
        (s, e) in rows(),
        domain in bounds(),
        point in -16i32..16,
    ) {
        let (mut ss, mut ee) = (s.clone(), e.clone());
        ss.extend([point, point]);
        ee.extend([point, point]);
        let duplicate = (0..s.len()).find(|&i| s[i] < e[i]);
        if let Some(i) = duplicate {
            ss.push(s[i]);
            ee.push(e[i]);
        }
        prop_assert_eq!(merge_intervals(&s, &e), merge_intervals(&ss, &ee));
        prop_assert_eq!(
            interval_gaps(&s, &e, domain.0, domain.1),
            interval_gaps(&ss, &ee, domain.0, domain.1)
        );
        for mode in [false, true] {
            let original = cluster_intervals(&s, &e, mode).unwrap();
            let updated = cluster_intervals(&ss, &ee, mode).unwrap();
            prop_assert_eq!(original.as_slice(), &updated[..s.len()]);
            let k = original.iter().copied().max().map_or(0, |v| v + 1);
            prop_assert_eq!(updated[s.len()], k);
            prop_assert_eq!(updated[s.len() + 1], k + 1);
            if let Some(i) = duplicate {
                prop_assert_eq!(*updated.last().unwrap(), original[i]);
            }
        }
    }

    #[test]
    fn union_idempotence_subsets_and_gaps_after_merging(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        let merged = merge_intervals(&s, &e).unwrap();
        let (ms, me) = unzip(&merged);
        prop_assert_eq!(&merge_intervals(&ms, &me).unwrap(), &merged);
        prop_assert_eq!(
            interval_gaps(&s, &e, domain.0, domain.1),
            interval_gaps(&ms, &me, domain.0, domain.1)
        );
        let pivot = s.len() / 2;
        let mut partial = merge_intervals(&s[..pivot], &e[..pivot]).unwrap();
        partial.extend(merge_intervals(&s[pivot..], &e[pivot..]).unwrap());
        let (ps, pe) = unzip(&partial);
        prop_assert_eq!(merge_intervals(&ps, &pe).unwrap(), merged);
    }

    #[test]
    fn splitting_rows_preserves_sets(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        let (mut ss, mut ee) = (Vec::new(), Vec::new());
        for (&a, &c) in s.iter().zip(&e) {
            let b = a + (c - a) / 2;
            ss.extend([a, b]);
            ee.extend([b, c]);
        }
        prop_assert_eq!(merge_intervals(&s, &e), merge_intervals(&ss, &ee));
        prop_assert_eq!(
            interval_gaps(&s, &e, domain.0, domain.1),
            interval_gaps(&ss, &ee, domain.0, domain.1)
        );
    }

    #[test]
    fn complement_partitions_domain_and_double_complement_is_clipped_union(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        let (l, r) = domain;
        let gaps = interval_gaps(&s, &e, l, r).unwrap();
        let (gs, ge) = unzip(&gaps);
        let clipped: Vec<_> = merge_intervals(&s, &e)
            .unwrap()
            .into_iter()
            .map(|(a, b)| (a.max(l), b.min(r)))
            .filter(|(a, b)| a < b)
            .collect();
        prop_assert_eq!(interval_gaps(&gs, &ge, l, r).unwrap(), clipped.clone());
        let area: i64 = clipped
            .iter()
            .chain(&gaps)
            .map(|&(a, b)| i64::from(b) - i64::from(a))
            .sum();
        prop_assert_eq!(area, i64::from(r) - i64::from(l));
        for tick in l..r {
            prop_assert_ne!(covered(&clipped, tick), covered(&gaps, tick));
        }
        let (as_, ae): (Vec<_>, Vec<_>) = clipped.iter().chain(&gaps).copied().unzip();
        let expected: Vec<_> = if l < r { vec![(l, r)] } else { vec![] };
        prop_assert_eq!(merge_intervals(&as_, &ae).unwrap(), expected);
    }

    #[test]
    fn adding_intervals_only_expands_coverage_and_shrinks_gap_sets(
        (s, e) in rows(),
        (more_s, more_e) in rows(),
        domain in bounds(),
    ) {
        let (mut ss, mut ee) = (s.clone(), e.clone());
        ss.extend(more_s);
        ee.extend(more_e);
        let before = merge_intervals(&s, &e).unwrap();
        let after = merge_intervals(&ss, &ee).unwrap();
        let gb = interval_gaps(&s, &e, domain.0, domain.1).unwrap();
        let ga = interval_gaps(&ss, &ee, domain.0, domain.1).unwrap();
        for tick in -16..16 {
            if covered(&before, tick) {
                prop_assert!(covered(&after, tick));
            }
            if covered(&ga, tick) {
                prop_assert!(covered(&gb, tick));
            }
        }
    }

    #[test]
    fn cluster_extents_and_unit_profile_agree_as_secondary_cross_checks(
        (s, e) in rows(),
        domain in bounds(),
    ) {
        let expected = merge_intervals(&s, &e).unwrap();
        for touching in [false, true] {
            let ids = cluster_intervals(&s, &e, touching).unwrap();
            let mut extents = std::collections::BTreeMap::<u32, (i32, i32)>::new();
            for i in 0..s.len() {
                if s[i] < e[i] {
                    extents
                        .entry(ids[i])
                        .and_modify(|r| {
                            r.0 = r.0.min(s[i]);
                            r.1 = r.1.max(e[i]);
                        })
                        .or_insert((s[i], e[i]));
                }
            }
            let mut extents: Vec<_> = extents.into_values().collect();
            extents.sort_unstable();
            if touching {
                prop_assert_eq!(&extents, &expected);
            }
            let (es, ee) = unzip(&extents);
            prop_assert_eq!(&merge_intervals(&es, &ee).unwrap(), &expected);
        }
        let profile = coverage_profile(&s, &e, None, false).unwrap();
        let (ps, pe): (Vec<_>, Vec<_>) = profile.iter().map(|p| (p.start, p.end)).unzip();
        prop_assert_eq!(merge_intervals(&ps, &pe).unwrap(), expected);
        let zero: Vec<_> = coverage_profile(&s, &e, Some(domain), true)
            .unwrap()
            .into_iter()
            .filter(|p| p.load == 0)
            .map(|p| (p.start, p.end))
            .collect();
        prop_assert_eq!(zero, interval_gaps(&s, &e, domain.0, domain.1).unwrap());
    }

    #[test]
    fn larger_collections_preserve_canonical_boundaries_and_subset_identity(
        pairs in prop::collection::vec((any::<i64>(), any::<i64>()), 0..300),
    ) {
        let (s, e): (Vec<_>, Vec<_>) = pairs.into_iter().map(|(a, b)| (a.min(b), a.max(b))).unzip();
        let output = merge_intervals(&s, &e).unwrap();
        canonical(&output);
        let boundaries: std::collections::BTreeSet<_> = s.iter().chain(&e).copied().collect();
        for &(a, b) in &output {
            prop_assert!(boundaries.contains(&a));
            prop_assert!(boundaries.contains(&b));
        }
        let gaps = interval_gaps(&s, &e, i64::MIN, i64::MAX).unwrap();
        canonical(&gaps);
        let (gs, ge) = unzip(&gaps);
        prop_assert_eq!(interval_gaps(&gs, &ge, i64::MIN, i64::MAX).unwrap(), output);
    }
}
