#[path = "../benches/support/clique.rs"]
mod candidates;

use intervals_core::IntervalError;

#[test]
fn all_candidates_check_only_actual_clique_objectives_for_overflow() {
    for method in candidates::METHODS {
        let run = |s: &[i32], e: &[i32], w: &[i128]| candidates::run(s, e, Some(w), method);
        assert_eq!(
            run(&[0, 1], &[3, 2], &[i128::MAX, 1]),
            Err(IntervalError::WeightOverflow),
            "{method}"
        );
        for (s, e) in [([0, 2], [1, 3]), ([0, 1], [1, 2]), ([0, 0], [0, 0])] {
            assert_eq!(
                run(&s, &e, &[i128::MAX, i128::MAX]).unwrap(),
                [true, false],
                "{method}"
            );
        }
        assert_eq!(
            run(&[0, 0, 0], &[2, 2, 0], &[i128::MIN, 5, 5]).unwrap(),
            [false, true, false],
            "{method}"
        );
        assert_eq!(
            candidates::run(
                &[0u64, 0],
                &[u64::MAX, u64::MAX],
                Some(&[u64::MAX, u64::MAX]),
                method
            )
            .unwrap(),
            [true, true],
            "{method}"
        );
    }
}

#[test]
fn all_candidates_validate_every_row_before_fast_paths() {
    for method in candidates::METHODS {
        assert_eq!(
            candidates::run(&[0, 4], &[0], Some(&[1i32, 1]), method),
            Err(IntervalError::LengthMismatch {
                starts_len: 2,
                ends_len: 1
            })
        );
        assert_eq!(
            candidates::run(&[0, 4], &[0, 4], Some(&[1i32]), method),
            Err(IntervalError::WeightLengthMismatch {
                intervals_len: 2,
                weights_len: 1
            })
        );
        for weights in [[0, 0, 0], [10, -5, -1], [1, 1, 1]] {
            assert_eq!(
                candidates::run(&[0, 1, 4], &[0, 2, 3], Some(&weights), method),
                Err(IntervalError::InvalidInterval { index: 2 }),
                "{method}"
            );
        }
        assert_eq!(
            candidates::run::<_, i64>(&['z', 'a', 'b'], &['z', 'd', 'c'], None, method).unwrap(),
            [false, true, true],
            "{method}"
        );
    }
}
