#[path = "../benches/support/mod.rs"]
mod support;

use intervals_core::IntervalError;
use proptest::prelude::*;
use support::{CANDIDATES, Candidate, optimum, shuffle, verify};

fn algorithms() -> impl Iterator<Item = (&'static str, Candidate)> {
    let production: Candidate =
        |starts, ends| intervals_core::assign_lanes(starts, ends).map(|v| (v, 0));
    CANDIDATES.into_iter().chain([("production", production)])
}

fn valid_intervals() -> impl Strategy<Value = (Vec<i64>, Vec<i64>)> {
    prop::collection::vec((-8i64..=8, 0i64..=8), 0..=30).prop_map(|intervals| {
        intervals
            .into_iter()
            .map(|(start, length)| (start, start + length))
            .unzip()
    })
}

fn check(starts: &[i64], ends: &[i64]) -> usize {
    let expected = optimum(starts, ends);
    for (_, run) in algorithms() {
        let (lanes, _) = run(starts, ends).unwrap();
        verify(starts, ends, &lanes, expected);
        // Independent O(n²) conflict check, in original row order.
        for i in 0..starts.len() {
            for j in 0..i {
                if starts[i] < ends[i]
                    && starts[j] < ends[j]
                    && starts[i] < ends[j]
                    && starts[j] < ends[i]
                {
                    assert_ne!(lanes[i], lanes[j], "rows {i} and {j} overlap");
                }
            }
        }
        assert_eq!(lanes, run(starts, ends).unwrap().0);
    }
    expected
}

#[test]
fn deterministic_cases() {
    for (starts, ends, expected) in [
        (vec![], vec![], 0),
        (vec![1], vec![3], 1),
        (vec![0, 1, 2], vec![1, 2, 3], 1),
        (vec![0, 1], vec![1, 2], 1),
        (vec![0, 1, 2, 3], vec![10, 9, 8, 7], 4),
        (vec![1; 4], vec![5; 4], 4),
        (vec![0, 1, 2, 3, 4], vec![2, 3, 4, 5, 6], 2),
        (vec![0, 1, 2, 3, 20, 21], vec![10, 9, 8, 7, 23, 24], 4),
        (vec![5], vec![5], 1),
        (vec![5; 30], vec![5; 30], 1),
        (vec![0, 0, 2, 4, 1], vec![4, 0, 2, 4, 3], 2),
        (vec![8, 3, 0, 1, 1], vec![9, 5, 10, 2, 2], 3),
        (vec![i64::MIN, 0, i64::MAX], vec![0, i64::MAX, i64::MAX], 1),
    ] {
        assert_eq!(check(&starts, &ends), expected);
    }
}

#[test]
fn equal_endpoints_and_shuffled_rows() {
    let mut intervals: Vec<_> = (0..100).map(|i| (i / 10, i / 10 + 1)).collect();
    shuffle(&mut intervals, &mut 42);
    let (starts, ends): (Vec<_>, Vec<_>) = intervals.into_iter().unzip();
    assert_eq!(check(&starts, &ends), 10);
}

#[test]
fn generic_endpoints_need_only_order_and_copy() {
    let lanes = intervals_core::assign_lanes(&['a', 'c', 'd'], &['d', 'f', 'd']).unwrap();
    assert_eq!(lanes.len(), 3);
    assert_ne!(lanes[0], lanes[1]);
    assert_eq!(lanes[2], 0);
    let lanes =
        intervals_core::assign_lanes(&[u64::MAX - 2, u64::MAX - 1], &[u64::MAX; 2]).unwrap();
    assert_ne!(lanes[0], lanes[1]);
}

#[test]
fn validation_uses_original_indices() {
    for (_, run) in algorithms() {
        assert_eq!(
            run(&[9, 0, 5, 3], &[10, 0, 4, 2]),
            Err(IntervalError::InvalidInterval { index: 2 })
        );
        for (starts, ends) in [(&[1][..], &[][..]), (&[][..], &[1][..])] {
            assert_eq!(
                run(starts, ends),
                Err(IntervalError::LengthMismatch {
                    starts_len: starts.len(),
                    ends_len: ends.len(),
                })
            );
        }
    }
}

proptest! {
    #[test]
    fn valid_optimal_contiguous_deterministic_and_correct_length((starts, ends) in valid_intervals()) {
        check(&starts, &ends);
    }

    #[test]
    fn translation_preserves_exact_assignment((starts, ends) in valid_intervals(), offset in -100i64..=100) {
        let shifted_starts: Vec<_> = starts.iter().map(|x| x + offset).collect();
        let shifted_ends: Vec<_> = ends.iter().map(|x| x + offset).collect();
        for (_, run) in algorithms() {
            prop_assert_eq!(run(&starts, &ends).unwrap().0, run(&shifted_starts, &shifted_ends).unwrap().0);
        }
    }

    #[test]
    fn permutation_preserves_optimum((starts, ends) in valid_intervals(), seed in 1u64..=u64::MAX) {
        let mut intervals: Vec<_> = starts.iter().copied().zip(ends.iter().copied()).collect();
        let mut state = seed;
        shuffle(&mut intervals, &mut state);
        let (permuted_starts, permuted_ends): (Vec<_>, Vec<_>) = intervals.into_iter().unzip();
        prop_assert_eq!(check(&starts, &ends), check(&permuted_starts, &permuted_ends));
    }

    #[test]
    fn adding_empty_does_not_increase_lanes((mut starts, mut ends) in valid_intervals(), x in -16i64..=16) {
        let expected = check(&starts, &ends).max(1);
        starts.push(x);
        ends.push(x);
        prop_assert_eq!(check(&starts, &ends), expected);
    }
}
