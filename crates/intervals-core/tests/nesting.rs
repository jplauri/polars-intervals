use intervals_core::{IntervalError, containment_counts, nesting_depths};
use proptest::prelude::*;

fn strictly_contains<T: Ord>(a: (&T, &T), b: (&T, &T)) -> bool {
    a.0 <= b.0 && b.1 <= a.1 && (a.0 < b.0 || b.1 < a.1)
}

// Independent O(n²) reference: recursively inspect the explicit relation.
// Memoization visits each row once; neither sorting nor rank logic is shared.
fn naive_depths<T: Ord>(starts: &[T], ends: &[T]) -> Vec<usize> {
    fn visit<T: Ord>(j: usize, starts: &[T], ends: &[T], memo: &mut [Option<usize>]) -> usize {
        if let Some(depth) = memo[j] {
            return depth;
        }
        let mut depth = 0;
        for i in 0..starts.len() {
            if strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j])) {
                depth = depth.max(visit(i, starts, ends, memo) + 1);
            }
        }
        memo[j] = Some(depth);
        depth
    }
    let mut memo = vec![None; starts.len()];
    (0..starts.len())
        .map(|j| visit(j, starts, ends, &mut memo))
        .collect()
}

// A completely separate tiny-instance oracle: enumerate subsets and recognize
// a chain by pairwise strict comparability. No longest-path recurrence is used.
fn brute_chain_cardinality<T: Ord>(starts: &[T], ends: &[T]) -> usize {
    (0usize..1 << starts.len())
        .filter(|&mask| {
            (0..starts.len()).all(|i| {
                (i + 1..starts.len()).all(|j| {
                    mask & (1 << i) == 0
                        || mask & (1 << j) == 0
                        || strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j]))
                        || strictly_contains((&starts[j], &ends[j]), (&starts[i], &ends[i]))
                })
            })
        })
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap_or(0)
}

fn valid_intervals() -> impl Strategy<Value = (Vec<i32>, Vec<i32>)> {
    // Both endpoints stay in [-8, 8], deliberately generating many ties/empties.
    prop::collection::vec((-8i32..=8, -8i32..=8), 0..=25)
        .prop_map(|pairs| pairs.into_iter().map(|(a, b)| (a.min(b), a.max(b))).unzip())
}

fn assert_depths(starts: &[i64], ends: &[i64], expected: &[usize]) {
    assert_eq!(naive_depths(starts, ends), expected);
    assert_eq!(nesting_depths(starts, ends).unwrap(), expected);
}

#[test]
fn empty_input() {
    assert_depths(&[], &[], &[]);
}

#[test]
fn single_interval() {
    assert_depths(&[1], &[5], &[0]);
}

#[test]
fn pairwise_disjoint() {
    assert_depths(&[0, 3, 7], &[2, 5, 9], &[0, 0, 0]);
}

#[test]
fn partial_crossings_are_not_nesting() {
    assert_depths(&[0, 3, 6], &[5, 8, 10], &[0, 0, 0]);
}

#[test]
fn deep_strict_chain() {
    assert_depths(&[0, 1, 2, 3], &[10, 9, 8, 7], &[0, 1, 2, 3]);
}

#[test]
fn equal_starts_still_chain() {
    assert_depths(&[1, 1, 1], &[10, 8, 5], &[0, 1, 2]);
}

#[test]
fn equal_ends_still_chain() {
    assert_depths(&[0, 2, 5], &[10, 10, 10], &[0, 1, 2]);
}

#[test]
fn identical_duplicates_never_chain() {
    assert_depths(&[1, 1, 1], &[5, 5, 5], &[0, 0, 0]);
}

#[test]
fn duplicates_inside_chain_share_one_level() {
    assert_depths(&[0, 2, 2, 3], &[10, 8, 8, 7], &[0, 1, 1, 2]);
}

#[test]
fn duplicate_outer_intervals_add_only_one_level() {
    assert_depths(&[0, 0, 2], &[10, 10, 8], &[0, 0, 1]);
}

#[test]
fn same_start_duplicate_group_plus_smaller_end() {
    assert_depths(&[1, 1, 1], &[10, 10, 8], &[0, 0, 1]);
}

#[test]
fn same_end_duplicate_group_plus_later_start() {
    assert_depths(&[0, 0, 2], &[10, 10, 10], &[0, 0, 1]);
}

#[test]
fn incomparable_containers_count_more_than_chain_depth() {
    let starts = [0, 1, 2, 4];
    let ends = [8, 9, 10, 5];
    assert_depths(&starts, &ends, &[0, 0, 0, 1]);
    let strict_containers = (0..3)
        .filter(|&i| strictly_contains((&starts[i], &ends[i]), (&starts[3], &ends[3])))
        .count();
    assert_eq!(strict_containers, 3);
    assert_eq!(containment_counts(&starts, &ends).unwrap(), [1, 1, 1, 0]);
}

#[test]
fn competing_chains_use_the_longest_container_path() {
    assert_depths(
        &[0, 1, 2, 3, 4, 5],
        &[10, 11, 9, 12, 8, 6],
        &[0, 0, 1, 0, 2, 3],
    );
}

#[test]
fn empty_interval_at_outer_end_is_strictly_contained() {
    assert_depths(&[0, 5], &[5, 5], &[0, 1]);
}

#[test]
fn identical_empty_intervals_do_not_chain() {
    assert_depths(&[3, 3], &[3, 3], &[0, 0]);
}

#[test]
fn ordinary_chain_ending_in_empty_accumulates_depth() {
    assert_depths(&[0, 2, 4, 5, 5], &[10, 8, 5, 5, 5], &[0, 1, 2, 3, 3]);
}

#[test]
fn empties_at_distinct_coordinates_are_incomparable() {
    assert_depths(&[-1, 0, 1, 2], &[-1, 0, 1, 2], &[0, 0, 0, 0]);
}

#[test]
fn empty_interval_at_outer_start_is_strictly_contained() {
    assert_depths(&[0, 0, 5, 6], &[5, 0, 5, 6], &[0, 1, 1, 0]);
}

#[test]
fn invalid_interval_reports_first_original_index() {
    assert_eq!(
        nesting_depths(&[8, 10, 3], &[9, 2, 1]),
        Err(IntervalError::InvalidInterval { index: 1 })
    );
}

#[test]
fn lengths_must_match_before_validating_intervals() {
    for (starts, ends) in [
        (&[2][..], &[][..]),
        (&[][..], &[1][..]),
        (&[2, 3][..], &[1][..]),
    ] {
        assert_eq!(
            nesting_depths(starts, ends),
            Err(IntervalError::LengthMismatch {
                starts_len: starts.len(),
                ends_len: ends.len(),
            })
        );
    }
}

#[test]
fn shuffled_chain_preserves_original_row_order() {
    assert_depths(&[3, 0, 2, 1], &[7, 10, 8, 9], &[3, 0, 2, 1]);
}

#[test]
fn reverse_sorted_chain_maps_back_to_input_order() {
    assert_depths(&[3, 2, 1, 0], &[7, 8, 9, 10], &[3, 2, 1, 0]);
}

#[test]
fn extreme_signed_endpoints_require_no_arithmetic() {
    assert_depths(
        &[i64::MIN, i64::MIN + 1, 0, i64::MAX],
        &[i64::MAX, i64::MAX, i64::MAX, i64::MAX],
        &[0, 1, 2, 3],
    );
}

#[test]
fn extreme_unsigned_endpoints_require_no_arithmetic() {
    let starts = [0u64, 1, u64::MAX - 1, u64::MAX];
    let ends = [u64::MAX; 4];
    assert_eq!(nesting_depths(&starts, &ends).unwrap(), [0, 1, 2, 3]);
    assert_eq!(naive_depths(&starts, &ends), [0, 1, 2, 3]);
}

#[test]
fn nonnumeric_ordered_endpoints() {
    assert_eq!(
        nesting_depths(&['a', 'b', 'c'], &['z', 'y', 'c']).unwrap(),
        [0, 1, 2]
    );
}

#[test]
fn large_deep_chain_does_not_recurse() {
    let starts: Vec<_> = (0..100_000).collect();
    let ends: Vec<_> = (100_000..200_000).rev().collect();
    assert_eq!(
        nesting_depths(&starts, &ends).unwrap(),
        (0..100_000).collect::<Vec<_>>()
    );
}

#[test]
fn exhaustive_ordered_tiny_collections_match_oracles() {
    let intervals: Vec<_> = (-1..=2)
        .flat_map(|start| (start..=2).map(move |end| (start, end)))
        .collect();
    // 111,111 ordered collections including repetition; every tie/permutation
    // pattern up to five rows occurs. Subset enumeration checks lengths <= 4.
    for len in 0..=5 {
        for mut code in 0..intervals.len().pow(len) {
            let mut starts = Vec::new();
            let mut ends = Vec::new();
            for _ in 0..len {
                let (start, end) = intervals[code % intervals.len()];
                code /= intervals.len();
                starts.push(start);
                ends.push(end);
            }
            let actual = nesting_depths(&starts, &ends).unwrap();
            assert_eq!(actual, naive_depths(&starts, &ends), "{starts:?} {ends:?}");
            if len <= 4 {
                assert_eq!(
                    actual.iter().max().map_or(0, |depth| depth + 1),
                    brute_chain_cardinality(&starts, &ends),
                    "{starts:?} {ends:?}"
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn prop_exact_agreement_with_naive_oracle((starts, ends) in valid_intervals()) {
        prop_assert_eq!(nesting_depths(&starts, &ends).unwrap(), naive_depths(&starts, &ends));
    }

    #[test]
    fn prop_output_length((starts, ends) in valid_intervals()) {
        prop_assert_eq!(nesting_depths(&starts, &ends).unwrap().len(), starts.len());
    }

    #[test]
    fn prop_depth_bounds((starts, ends) in valid_intervals()) {
        for depth in nesting_depths(&starts, &ends).unwrap() {
            prop_assert!(depth < starts.len());
        }
    }

    #[test]
    fn prop_strict_container_lower_bound((starts, ends) in valid_intervals()) {
        let depths = nesting_depths(&starts, &ends).unwrap();
        for i in 0..starts.len() {
            for j in 0..starts.len() {
                if strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j])) {
                    prop_assert!(depths[j] > depths[i]);
                }
            }
        }
    }

    #[test]
    fn prop_zero_depth_iff_no_strict_container((starts, ends) in valid_intervals()) {
        let depths = nesting_depths(&starts, &ends).unwrap();
        for j in 0..starts.len() {
            let has_container = (0..starts.len())
                .any(|i| strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j])));
            prop_assert_eq!(depths[j] == 0, !has_container);
        }
    }

    #[test]
    fn prop_permutation_equivariance(
        (starts, ends) in valid_intervals(),
        priorities in prop::collection::vec(any::<u64>(), 25),
    ) {
        let depths = nesting_depths(&starts, &ends).unwrap();
        let mut order: Vec<_> = (0..starts.len()).collect();
        order.sort_unstable_by_key(|&i| (priorities[i], i));
        let shuffled_starts: Vec<_> = order.iter().map(|&i| starts[i]).collect();
        let shuffled_ends: Vec<_> = order.iter().map(|&i| ends[i]).collect();
        let shuffled = nesting_depths(&shuffled_starts, &shuffled_ends).unwrap();
        for (j, &i) in order.iter().enumerate() {
            prop_assert_eq!(shuffled[j], depths[i]);
        }
    }

    #[test]
    fn prop_translation_invariance((starts, ends) in valid_intervals(), offset in -100i32..=100) {
        let translated_starts: Vec<_> = starts.iter().map(|s| s + offset).collect();
        let translated_ends: Vec<_> = ends.iter().map(|e| e + offset).collect();
        prop_assert_eq!(nesting_depths(&translated_starts, &translated_ends).unwrap(), nesting_depths(&starts, &ends).unwrap());
    }

    #[test]
    fn prop_positive_scaling_invariance((starts, ends) in valid_intervals(), factor in 1i32..=100) {
        let scaled_starts: Vec<_> = starts.iter().map(|s| s * factor).collect();
        let scaled_ends: Vec<_> = ends.iter().map(|e| e * factor).collect();
        prop_assert_eq!(nesting_depths(&scaled_starts, &scaled_ends).unwrap(), nesting_depths(&starts, &ends).unwrap());
    }

    #[test]
    fn prop_reflection_invariance((starts, ends) in valid_intervals()) {
        let reflected_starts: Vec<_> = ends.iter().map(|e| -e).collect();
        let reflected_ends: Vec<_> = starts.iter().map(|s| -s).collect();
        prop_assert_eq!(nesting_depths(&reflected_starts, &reflected_ends).unwrap(), nesting_depths(&starts, &ends).unwrap());
    }

    #[test]
    fn prop_duplicate_insertion_invariance((mut starts, mut ends) in valid_intervals(), choice in any::<usize>()) {
        if !starts.is_empty() {
            let original = nesting_depths(&starts, &ends).unwrap();
            let index = choice % starts.len();
            starts.push(starts[index]);
            ends.push(ends[index]);
            let actual = nesting_depths(&starts, &ends).unwrap();
            prop_assert_eq!(&actual[..original.len()], &original);
            prop_assert_eq!(actual[original.len()], original[index]);
        }
    }

    #[test]
    fn prop_expansion_cannot_increase_own_depth(
        (mut starts, mut ends) in valid_intervals(), choice in any::<usize>(),
        left in 0i32..=10, right in 0i32..=10,
    ) {
        if !starts.is_empty() {
            let index = choice % starts.len();
            let old = nesting_depths(&starts, &ends).unwrap()[index];
            starts[index] -= left;
            ends[index] += right;
            prop_assert!(nesting_depths(&starts, &ends).unwrap()[index] <= old);
        }
    }

    #[test]
    fn prop_shrinking_cannot_decrease_own_depth(
        (mut starts, mut ends) in valid_intervals(), choice in any::<usize>(),
        left in 0i32..=16, right in 0i32..=16,
    ) {
        if !starts.is_empty() {
            let index = choice % starts.len();
            let old = nesting_depths(&starts, &ends).unwrap()[index];
            starts[index] += left.min(ends[index] - starts[index]);
            ends[index] -= right.min(ends[index] - starts[index]);
            prop_assert!(nesting_depths(&starts, &ends).unwrap()[index] >= old);
        }
    }

    #[test]
    fn prop_deep_chain_exactness(n in 0i32..=250, stride in 1i32..=10) {
        let starts: Vec<_> = (0..n).map(|i| i * stride).collect();
        let ends: Vec<_> = (0..n).map(|i| (2 * n - i) * stride).collect();
        prop_assert_eq!(nesting_depths(&starts, &ends).unwrap(), (0..n as usize).collect::<Vec<_>>());
    }

    #[test]
    fn prop_disjoint_and_crossing_antichains(n in 0i32..=100) {
        let starts: Vec<_> = (0..n).map(|i| i * 3).collect();
        for width in [1, 3 * n + 1] {
            let ends: Vec<_> = starts.iter().map(|s| s + width).collect();
            prop_assert_eq!(nesting_depths(&starts, &ends).unwrap(), vec![0; n as usize]);
        }
    }

    #[test]
    fn prop_identical_family(n in 0usize..=100, start in -8i32..=8, length in 0i32..=8) {
        prop_assert_eq!(nesting_depths(&vec![start; n], &vec![start + length; n]).unwrap(), vec![0; n]);
    }

    #[test]
    fn prop_duplicate_chain_exact_levels(repetitions in prop::collection::vec(1usize..=8, 0..=25)) {
        let n = repetitions.len() as i32;
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        let mut expected = Vec::new();
        for (level, repeats) in repetitions.into_iter().enumerate() {
            starts.extend(std::iter::repeat_n(level as i32, repeats));
            ends.extend(std::iter::repeat_n(2 * n - level as i32, repeats));
            expected.extend(std::iter::repeat_n(level, repeats));
        }
        prop_assert_eq!(nesting_depths(&starts, &ends).unwrap(), expected);
    }

    #[test]
    fn prop_depth_bounded_by_strict_container_count((starts, ends) in valid_intervals()) {
        let depths = nesting_depths(&starts, &ends).unwrap();
        for (j, &depth) in depths.iter().enumerate() {
            let count = (0..starts.len())
                .filter(|&i| strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j])))
                .count();
            prop_assert!(depth <= count);
        }
    }

    #[test]
    fn prop_global_maximum_matches_brute_force_chain_cardinality(
        pairs in prop::collection::vec((-3i32..=3, -3i32..=3), 0..=8),
    ) {
        let (starts, ends): (Vec<_>, Vec<_>) = pairs.into_iter().map(|(a, b)| (a.min(b), a.max(b))).unzip();
        let depths = nesting_depths(&starts, &ends).unwrap();
        prop_assert_eq!(depths.iter().max().map_or(0, |depth| depth + 1), brute_chain_cardinality(&starts, &ends));
    }

    #[test]
    fn prop_containment_counts_share_endpoint_semantics((starts, ends) in valid_intervals()) {
        let counts = containment_counts(&starts, &ends).unwrap();
        let depths = nesting_depths(&starts, &ends).unwrap();
        let mut total_strict_containers = 0;
        for i in 0..starts.len() {
            let strict_children: Vec<_> = (0..starts.len())
                .filter(|&j| strictly_contains((&starts[i], &ends[i]), (&starts[j], &ends[j])))
                .collect();
            let identical_others = (0..starts.len())
                .filter(|&j| i != j && starts[i] == starts[j] && ends[i] == ends[j])
                .count();
            prop_assert_eq!(counts[i], strict_children.len() + identical_others);
            for j in strict_children {
                prop_assert!(depths[j] > depths[i]);
                total_strict_containers += 1;
            }
        }
        prop_assert!(depths.iter().sum::<usize>() <= total_strict_containers);
    }
}
