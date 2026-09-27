#[allow(dead_code)]
#[path = "../benches/support/nesting_depth.rs"]
mod candidates;
use candidates::{Fenwick, MaxTree, Segment};
use proptest::prelude::*;

fn boundaries<T: MaxTree>() {
    assert_eq!(T::new(0).prefix(0), None);
    let mut tree = T::new(5);
    assert_eq!(tree.prefix(5), None);
    tree.update(4, 0);
    assert_eq!(tree.prefix(4), None);
    assert_eq!(tree.prefix(5), Some(0));
    tree.update(0, 3);
    tree.update(0, 1);
    tree.update(0, 3);
    assert_eq!(tree.prefix(0), None);
    assert_eq!(tree.prefix(1), Some(3));
    assert_eq!(tree.prefix(5), Some(3));
    tree.update(4, 9);
    assert_eq!(tree.prefix(4), Some(3));
    assert_eq!(tree.prefix(5), Some(9));
}

#[test]
fn fenwick_absence_depth_zero_boundaries_repeated_and_lower_updates() {
    boundaries::<Fenwick>();
}

#[test]
fn segment_absence_depth_zero_boundaries_repeated_and_lower_updates() {
    boundaries::<Segment>();
}

#[test]
fn duplicate_batch_and_equal_start_visibility_all_candidates() {
    for (s, e, expected) in [
        (vec![1, 1, 1], vec![10, 10, 8], vec![0, 0, 1]),
        (vec![0, 0, 2], vec![10, 10, 10], vec![0, 0, 1]),
        (vec![0, 2, 2, 3], vec![10, 8, 8, 7], vec![0, 1, 1, 2]),
        (vec![5, 0], vec![5, 5], vec![1, 0]),
    ] {
        for method in candidates::METHODS {
            assert_eq!(
                candidates::run(&s, &e, method).unwrap().depths,
                expected,
                "{method}"
            );
        }
    }
}

#[test]
fn every_benchmark_structure_agrees_with_independent_oracle() {
    for scenario in candidates::SCENARIOS {
        let (s, e) = candidates::dataset(scenario, 101);
        let expected = candidates::naive(&s, &e);
        for method in candidates::METHODS {
            assert_eq!(
                candidates::run(&s, &e, method).unwrap().depths,
                expected,
                "{scenario}/{method}"
            );
        }
    }
}

#[test]
fn reversed_end_compression_extremes() {
    let (ends, ranks) = candidates::compressed_ranks(&[i64::MAX, 0, i64::MIN, i64::MAX]);
    assert_eq!(ends, [i64::MIN, 0, i64::MAX]);
    assert_eq!(ranks, [0, 1, 2, 0]);
}

#[test]
fn reversed_ranks_turn_endpoint_suffix_into_inclusive_prefix() {
    let ends = [10, 8, 8, 5];
    let depths = [0, 4, 2, 7];
    let (coordinates, ranks) = candidates::compressed_ranks(&ends);
    let mut fenwick = Fenwick::new(coordinates.len());
    let mut segment = Segment::new(coordinates.len());
    for (&rank, depth) in ranks.iter().zip(depths) {
        fenwick.update(rank, depth);
        segment.update(rank, depth);
    }
    for (&end, rank) in ends.iter().zip(ranks) {
        let expected = ends
            .iter()
            .zip(depths)
            .filter_map(|(&e, d)| (e >= end).then_some(d))
            .max();
        assert_eq!(fenwick.prefix(rank + 1), expected);
        assert_eq!(segment.prefix(rank + 1), expected);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn all_candidates_agree_with_literal_predicate_oracle(rows in prop::collection::vec((-8i32..=8, -8i32..=8), 0..=25)) {
        let (s, e): (Vec<_>, Vec<_>) = rows.into_iter().map(|(a,b)| (a.min(b), a.max(b))).unzip();
        let expected = candidates::naive(&s, &e);
        for method in candidates::METHODS {
            prop_assert_eq!(&candidates::run(&s, &e, method).unwrap().depths, &expected);
        }
    }

    #[test]
    fn max_trees_random_update_query_traces(
        n in 1usize..=40,
        operations in prop::collection::vec((any::<usize>(), any::<bool>(), 0usize..=100), 0..200),
    ) {
        let mut fenwick = Fenwick::new(n);
        let mut segment = Segment::new(n);
        let mut values = vec![None; n];
        for (index, update, depth) in operations {
            if update {
                let i = index % n;
                values[i] = values[i].max(Some(depth));
                fenwick.update(i, depth);
                segment.update(i, depth);
            } else {
                let end = index % (n + 1);
                let expected = values[..end].iter().copied().flatten().max();
                prop_assert_eq!(fenwick.prefix(end), expected);
                prop_assert_eq!(segment.prefix(end), expected);
            }
        }
    }

    #[test]
    fn reversed_rank_mapping_matches_naive_unique_ends(ends in prop::collection::vec(-8i64..=8, 0..=40)) {
        let (coordinates, ranks) = candidates::compressed_ranks(&ends);
        let expected: Vec<_> = ends.iter().copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        prop_assert_eq!(&coordinates, &expected);
        for (end, rank) in ends.iter().zip(ranks) {
            prop_assert_eq!(rank, expected.iter().filter(|other| *other > end).count());
        }
    }
}
