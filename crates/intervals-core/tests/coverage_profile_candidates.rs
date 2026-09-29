#[path = "../benches/support/coverage_profile_candidates.rs"]
mod candidates;

use candidates::{METHODS, oracle, run};
use intervals_core::{CoverageSegment, IntervalError};
use proptest::prelude::*;

fn check<T: Ord + Copy + std::fmt::Debug>(s: &[T], e: &[T], w: &[i128], domain: Option<(T, T)>) {
    for zero in [false, true] {
        for weights in [None, Some(w)] {
            let expected = oracle(s, e, weights, domain, zero);
            for method in METHODS {
                assert_eq!(
                    run(method, s, e, weights, domain, zero),
                    expected,
                    "{method}"
                );
            }
        }
    }
}

#[test]
fn empty_zero_gap_ties_and_full_heap_tail() {
    for domain in [None, Some((-10, 40)), Some((2, 2)), Some((3, 6))] {
        check::<i64>(&[], &[], &[], domain);
        check(
            &[0, 1, 2, 30],
            &[0, 1, 2, 30],
            &[4, 0, i128::MAX, 5],
            domain,
        );
        check(&[0, 2, 5, 3], &[4, 5, 7, 3], &[2, 3, 3, 8], domain);
        check(&[0, 4], &[2, 6], &[7, 7], domain);
        check(
            &[0, 0, 0, 1, 2, 3],
            &[7, 6, 4, 5, 3, 4],
            &[3, 2, 1, 0, 3, 3],
            domain,
        );
        check(
            &[0, 10, -20, 200],
            &[100, 20, -20, 200],
            &[0, 7, 8, 9],
            domain,
        );
    }
}

#[test]
fn all_candidates_accept_generic_extreme_endpoints_and_exact_loads() {
    check(
        &['a', 'c', 'b'],
        &['c', 'd', 'c'],
        &[8, 8, 0],
        Some(('a', 'z')),
    );
    check(&[i64::MIN, 0], &[0, i64::MAX], &[i128::MAX; 2], None);
    check(&[0, u64::MAX - 1], &[1, u64::MAX], &[i128::MAX; 2], None);
    check(&[i8::MIN, 0], &[0, i8::MAX], &[7, 7], None);
    check(&[0, 0], &[2, 2], &[i128::from(u64::MAX); 2], None);
    check(&[0, 0], &[2, 1], &[i128::MAX, 1], Some((1, 2)));
    check(&[0, 0], &[2, 1], &[i128::MAX, 1], Some((1, 1)));
    check(&[0, 0], &[2, 1], &[i128::MAX, 1], None);
}

#[test]
fn all_candidates_validate_original_indices_before_pruning() {
    for method in METHODS {
        for domain in [None, Some((0, 0)), Some((10, 20))] {
            assert_eq!(
                run(
                    method,
                    &[0, 2],
                    &[0, 1],
                    Some(&[0i128, 0][..]),
                    domain,
                    false
                ),
                Err(IntervalError::InvalidInterval { index: 1 })
            );
            assert_eq!(
                run(
                    method,
                    &[0, 2],
                    &[0, 2],
                    Some(&[0i128, -1][..]),
                    domain,
                    false
                ),
                Err(IntervalError::NegativeLoad { index: 1 })
            );
        }
        assert!(matches!(
            run(method, &[0], &[], None::<&[i128]>, None, false),
            Err(IntervalError::LengthMismatch(_))
        ));
        assert!(matches!(
            run(method, &[0], &[1], Some(&[] as &[i128]), None, false),
            Err(IntervalError::LengthMismatch(_))
        ));
        assert_eq!(
            run(method, &[0], &[0], None::<&[i128]>, Some((2, 1)), false),
            Err(IntervalError::InvalidDomain)
        );
    }
}

#[test]
fn tiny_per_tick_oracle_independently_checks_breakpoints() {
    let (starts, ends, weights) = ([4, 1, -3, 0, 2], [7, 7, 0, 2, 2], [3i128, 0, 4, 4, 99]);
    for method in METHODS {
        let profile = run(method, &starts, &ends, Some(&weights), Some((-5, 10)), true).unwrap();
        for tick in -5..10 {
            let expected: i128 = (0..starts.len())
                .filter(|&i| starts[i] <= tick && tick < ends[i])
                .map(|i| weights[i])
                .sum();
            let actual = profile
                .iter()
                .find(|p| p.start <= tick && tick < p.end)
                .unwrap();
            assert_eq!(actual.load, expected, "{method}, tick={tick}");
        }
    }
}

proptest! {
    #[test]
    fn every_candidate_matches_membership_and_tick_oracles(
        rows in prop::collection::vec((-12i64..=12, -12i64..=12, 0i128..=9), 0..40),
        bounds in prop::option::of((-15i64..=15, -15i64..=15)),
        zero in any::<bool>(),
    ) {
        let (s, e): (Vec<_>, Vec<_>) = rows.iter().map(|&(a,b,_)| (a.min(b), a.max(b))).unzip();
        let weights: Vec<_> = rows.iter().map(|r| r.2).collect();
        let domain = bounds.map(|(a,b)| (a.min(b), a.max(b)));
        for w in [None, Some(weights.as_slice())] {
            let expected = oracle(&s, &e, w, domain, zero).unwrap();
            for method in METHODS {
                let actual = run(method, &s, &e, w, domain, zero).unwrap();
                prop_assert_eq!(&actual, &expected);
                prop_assert!(actual.len() <= 2 * s.len() + 1);
                for segment in &actual {
                    prop_assert!(segment.start < segment.end);
                    prop_assert!(segment.load >= i128::from(!zero));
                    for point in [segment.start, segment.end] {
                        prop_assert!(s.contains(&point) || e.contains(&point)
                            || domain.is_some_and(|(a,b)| point == a || point == b));
                    }
                }
                for pair in actual.windows(2) {
                    prop_assert!(pair[0].end <= pair[1].start);
                    prop_assert!(pair[0].end != pair[1].start || pair[0].load != pair[1].load);
                }
                for tick in -15..15 {
                    let in_domain = domain.is_none_or(|(a,b)| a <= tick && tick < b);
                    let want: i128 = if in_domain {
                        (0..s.len()).filter(|&i| s[i] <= tick && tick < e[i]).map(|i| w.map_or(1, |w| w[i])).sum()
                    } else { 0 };
                    let got = actual.iter().find(|p| p.start <= tick && tick < p.end).map_or(0, |p| p.load);
                    prop_assert_eq!(got, want);
                }
                let reversed_s: Vec<_> = s.iter().rev().copied().collect();
                let reversed_e: Vec<_> = e.iter().rev().copied().collect();
                let reversed_w: Vec<_> = weights.iter().rev().copied().collect();
                let reversed = run(method, &reversed_s, &reversed_e, w.map(|_| reversed_w.as_slice()), domain, zero).unwrap();
                prop_assert_eq!(&actual, &reversed);
                let profile_s: Vec<_> = actual.iter().map(|p| p.start).collect();
                let profile_e: Vec<_> = actual.iter().map(|p| p.end).collect();
                let profile_w: Vec<_> = actual.iter().map(|p| p.load).collect();
                let repeated = run(method, &profile_s, &profile_e, Some(profile_w.as_slice()), domain, zero).unwrap();
                prop_assert_eq!(&actual, &repeated);
            }
        }
    }

    #[test]
    fn every_candidate_preserves_relabeling_translation_and_quantity_scaling(
        rows in prop::collection::vec((-10i64..=10, -10i64..=10, 0i128..=9), 0..40),
        bounds in prop::option::of((-15i64..=15, -15i64..=15)),
        zero in any::<bool>(),
        factor in 1i128..=7,
        shift in -100i64..=100,
    ) {
        let (s,e): (Vec<_>,Vec<_>) = rows.iter().map(|&(a,b,_)| (a.min(b),a.max(b))).unzip();
        let weights: Vec<_> = rows.iter().map(|r| r.2).collect();
        let domain = bounds.map(|(a,b)| (a.min(b),a.max(b)));
        let relabel = |t:i64| t*t*t + t;
        for method in METHODS {
            for w in [None,Some(weights.as_slice())] {
                let expected = run(method,&s,&e,w,domain,zero).unwrap();
                for map in [&relabel as &dyn Fn(i64)->i64,&|t|t+shift] {
                    let changed_s: Vec<_> = s.iter().copied().map(map).collect();
                    let changed_e: Vec<_> = e.iter().copied().map(map).collect();
                    let changed_domain = domain.map(|(a,b)| (map(a),map(b)));
                    let actual = run(method,&changed_s,&changed_e,w,changed_domain,zero).unwrap();
                    let changed_expected: Vec<_> = expected.iter().map(|p| CoverageSegment { start:map(p.start),end:map(p.end),load:p.load }).collect();
                    prop_assert_eq!(actual,changed_expected);
                }
                let scaled_weights: Vec<_> = (0..s.len()).map(|i| w.map_or(1,|w|w[i])*factor).collect();
                let actual = run(method,&s,&e,Some(&scaled_weights),domain,zero).unwrap();
                let scaled_expected: Vec<_> = expected.iter().map(|p| CoverageSegment { load:p.load*factor,..*p }).collect();
                prop_assert_eq!(actual,scaled_expected);
            }
        }
    }

    #[test]
    fn every_candidate_preserves_splitting_empty_rows_and_fixed_domain_zero_additions(
        rows in prop::collection::vec((-10i64..=10, -10i64..=10, 0i128..=9), 0..40),
        bounds in prop::option::of((-15i64..=15, -15i64..=15)),
        zero in any::<bool>(),
    ) {
        let (s,e): (Vec<_>,Vec<_>) = rows.iter().map(|&(a,b,_)| (a.min(b),a.max(b))).unzip();
        let weights: Vec<_> = rows.iter().map(|r| r.2).collect();
        let domain = bounds.map(|(a,b)| (a.min(b),a.max(b)));
        for method in METHODS {
            for w in [None,Some(weights.as_slice())] {
                let expected = run(method,&s,&e,w,domain,zero).unwrap();
                let (mut split_s,mut split_e,mut split_w) = (Vec::new(),Vec::new(),Vec::new());
                for i in 0..s.len() {
                    let q = w.map_or(1,|w|w[i]);
                    if e[i]-s[i]>1 {
                        let mid = s[i]+(e[i]-s[i])/2;
                        split_s.extend([s[i],mid]); split_e.extend([mid,e[i]]); split_w.extend([q,q]);
                    } else { split_s.push(s[i]);split_e.push(e[i]);split_w.push(q); }
                }
                split_s.extend([-100,100]);split_e.extend([-100,100]);split_w.extend([i128::MAX,i128::MAX]);
                let actual = run(method,&split_s,&split_e,Some(&split_w),domain,zero).unwrap();
                prop_assert_eq!(actual,expected);
                let fixed = Some((-25,25));
                let fixed_expected = run(method,&s,&e,w,fixed,zero).unwrap();
                split_s.push(-30);split_e.push(30);split_w.push(0);
                prop_assert_eq!(run(method,&split_s,&split_e,Some(&split_w),fixed,zero).unwrap(),fixed_expected);
                // Without fixed bounds the zero row can enlarge the full-mode
                // domain; only the represented load function stays invariant.
                let before = run(method,&s,&e,w,None,zero).unwrap();
                let after = run(method,&split_s,&split_e,Some(&split_w),None,zero).unwrap();
                for tick in -30..30 { prop_assert_eq!(load_at(&before,tick),load_at(&after,tick)); }
                if zero { prop_assert_eq!(after.first().unwrap().start,-30);prop_assert_eq!(after.last().unwrap().end,30); }
            }
        }
    }

    #[test]
    fn every_candidate_respects_partition_addition_restriction_sparse_filter_and_area(
        rows in prop::collection::vec((-10i64..=10, -10i64..=10, 0i128..=9, any::<bool>()), 0..40),
        bounds in (-15i64..=15, -15i64..=15),
    ) {
        let (s,e): (Vec<_>,Vec<_>) = rows.iter().map(|&(a,b,_,_)| (a.min(b),a.max(b))).unzip();
        let weights: Vec<_> = rows.iter().map(|r|r.2).collect();
        let domain = Some((-15,15));
        for method in METHODS {
            for w in [None,Some(weights.as_slice())] {
                let full = run(method,&s,&e,w,domain,true).unwrap();
                let positive: Vec<_> = full.iter().copied().filter(|p|p.load>0).collect();
                prop_assert_eq!(run(method,&s,&e,w,domain,false).unwrap(),positive);
                let (mut partial_s,mut partial_e,mut partial_w) = (Vec::new(),Vec::new(),Vec::new());
                for selector in [false,true] {
                    let indices: Vec<_> = rows.iter().enumerate().filter_map(|(i,r)| (r.3==selector).then_some(i)).collect();
                    let subset_s: Vec<_> = indices.iter().map(|&i|s[i]).collect();
                    let subset_e: Vec<_> = indices.iter().map(|&i|e[i]).collect();
                    let subset_w: Vec<_> = indices.iter().map(|&i|w.map_or(1,|w|w[i])).collect();
                    let subset = run(method,&subset_s,&subset_e,Some(&subset_w),domain,true).unwrap();
                    for p in subset { partial_s.push(p.start);partial_e.push(p.end);partial_w.push(p.load); }
                }
                // The membership oracle adds profiles pointwise and independently
                // coalesces them, rather than concatenating partial output blocks.
                prop_assert_eq!(&oracle(&partial_s,&partial_e,Some(&partial_w),domain,true).unwrap(),&full);
                let smaller = Some((bounds.0.min(bounds.1),bounds.0.max(bounds.1)));
                let ps: Vec<_> = full.iter().map(|p|p.start).collect();
                let pe: Vec<_> = full.iter().map(|p|p.end).collect();
                let pw: Vec<_> = full.iter().map(|p|p.load).collect();
                for zero in [false,true] {
                    prop_assert_eq!(run(method,&ps,&pe,Some(&pw),smaller,zero).unwrap(),run(method,&s,&e,w,smaller,zero).unwrap());
                }
                let output_area: i128 = full.iter().map(|p|p.load*(i128::from(p.end)-i128::from(p.start))).sum();
                let input_area: i128 = (0..s.len()).map(|i|w.map_or(1,|w|w[i])*(i128::from(e[i])-i128::from(s[i]))).sum();
                prop_assert_eq!(output_area,input_area);
            }
        }
    }
}

fn load_at(profile: &[CoverageSegment<i64>], t: i64) -> i128 {
    profile
        .iter()
        .find(|p| p.start <= t && t < p.end)
        .map_or(0, |p| p.load)
}
