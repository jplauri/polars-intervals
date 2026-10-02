#[path = "../benches/support/coverage_stats_candidates.rs"]
mod candidates;
// The candidate shares production preparation without exposing a public index.
#[allow(dead_code)]
#[path = "../src/geometry.rs"]
mod geometry;
#[path = "support/coverage_stats.rs"]
mod oracle;

use intervals_core::{
    CoverageEndpoint, CoverageStats, CoverageStatsError, IntervalError, coverage_stats,
    intersect_intervals, merge_intervals, overlap_counts, subtract_intervals, validate_intervals,
};
use proptest::prelude::*;

fn solve<T: CoverageEndpoint>(queries: &[(T, T)], sources: &[(T, T)]) -> Vec<CoverageStats> {
    let (qs, qe): (Vec<_>, Vec<_>) = queries.iter().copied().unzip();
    let (ss, se): (Vec<_>, Vec<_>) = sources.iter().copied().unzip();
    coverage_stats(&qs, &qe, &ss, &se).unwrap()
}

fn check<T: CoverageEndpoint + std::fmt::Debug>(
    queries: &[(T, T)],
    sources: &[(T, T)],
) -> Vec<CoverageStats> {
    let (qs, qe): (Vec<_>, Vec<_>) = queries.iter().copied().unzip();
    let (ss, se): (Vec<_>, Vec<_>) = sources.iter().copied().unzip();
    let expected = oracle::cells(&qs, &qe, &ss, &se);
    for method in candidates::METHODS {
        let actual = candidates::run(method, &qs, &qe, &ss, &se).unwrap();
        assert_eq!(
            actual, expected,
            "{method}: queries={queries:?}, sources={sources:?}"
        );
        assert_eq!(actual.len(), queries.len());
        for row in &actual {
            assert!(row.overlap_count <= sources.len() as u64);
            assert!(0 <= row.covered_length && row.covered_length <= row.query_length);
            assert_eq!(row.overlap_count == 0, row.covered_length == 0);
            if row.query_length == 0 {
                assert_eq!(
                    *row,
                    CoverageStats {
                        overlap_count: 0,
                        covered_length: 0,
                        query_length: 0,
                        covered_fraction: None
                    }
                );
            }
        }
    }
    expected
}

#[test]
fn canonical_example_duplicates_and_query_order() {
    let rows = check(
        &[(7, 7), (5, 10), (0, 10), (12, 15), (5, 10)],
        &[(1, 7), (4, 9)],
    );
    assert_eq!(
        rows.iter()
            .map(|r| (r.overlap_count, r.covered_length))
            .collect::<Vec<_>>(),
        [(0, 0), (2, 4), (2, 8), (0, 0), (2, 4)]
    );
    assert_eq!(rows[1].covered_fraction, Some(0.8));
    assert_eq!(rows[0].covered_fraction, None);
}

#[test]
fn empty_operands_sources_and_queries() {
    for sources in [vec![], vec![(0, 0), (2, 2), (9, 9)], vec![(0, 9)]] {
        check::<i64>(&[], &sources);
        check(&[(0, 0), (1, 1), (9, 9), (10, 10)], &sources);
        check(&[(-2, 12)], &sources);
    }
}

#[test]
fn touching_ties_count_rows_not_runs_or_depth() {
    assert_eq!(check(&[(0, 2)], &[(2, 4)])[0].overlap_count, 0);
    assert_eq!(check(&[(0, 4)], &[(0, 2), (2, 4)])[0].overlap_count, 2);
    let row = check(&[(0, 10)], &[(0, 2), (3, 5), (6, 8)])[0];
    assert_eq!((row.overlap_count, row.covered_length), (3, 6));
    let row = check(&[(0, 10)], &[(1, 7), (1, 7)])[0];
    assert_eq!((row.overlap_count, row.covered_length), (2, 6));
    check(
        &[(2, 4), (4, 8), (0, 2), (8, 9), (1, 5)],
        &[(0, 2), (0, 2), (2, 4), (2, 4), (4, 8), (8, 8)],
    );
}

#[test]
fn gaps_partial_hits_nested_sources_and_decreasing_query_ends() {
    check(
        &[
            (-9, -1),
            (-2, 30),
            (0, 20),
            (1, 18),
            (2, 16),
            (3, 14),
            (4, 12),
            (7, 8),
            (30, 50),
        ],
        &[
            (20, 25),
            (2, 4),
            (10, 12),
            (1, 6),
            (3, 5),
            (8, 10),
            (16, 18),
        ],
    );
    check(
        &[(0, 10), (1, 9), (2, 8), (3, 7)],
        &[(0, 100), (2, 4), (2, 4), (3, 3)],
    );
}

#[test]
fn exact_extreme_lengths_and_prefix_cancellation() {
    let full = i128::from(u64::MAX);
    assert_eq!(
        check(&[(i64::MIN, i64::MAX)], &[(i64::MIN, i64::MAX)])[0].covered_length,
        full
    );
    assert_eq!(
        check(&[(0, u64::MAX)], &[(0, u64::MAX)])[0].query_length,
        full
    );
    assert_eq!(
        check(&[(i8::MIN, i8::MAX)], &[(i8::MIN, i8::MAX)])[0].covered_length,
        255
    );
    let x = (1u64 << 54) + 7;
    let rows = check(
        &[(x, x + 1), (x + 1, x + 2), (0, u64::MAX)],
        &[(0, x), (x, x + 1), (x + 2, u64::MAX)],
    );
    assert_eq!(rows[0].covered_length, 1);
    assert_eq!(rows[1].covered_length, 0);
    assert_eq!(rows[2].covered_length, full - 1);
    assert_eq!(rows[2].covered_fraction, Some(1.0));
}

#[test]
fn validate_original_rows_before_empty_or_irrelevant_shortcuts() {
    for method in candidates::METHODS {
        for (qs, qe) in [(vec![], vec![]), (vec![0], vec![0]), (vec![0], vec![1])] {
            let error = candidates::run(method, &qs, &qe, &[99, 8, 100], &[99, 9, 90]).unwrap_err();
            assert_eq!(
                error,
                CoverageStatsError::Intervals(IntervalError::InvalidInterval { index: 2 })
            );
            assert!(error.to_string().contains("intervals"));
        }
        assert_eq!(
            candidates::run(method, &[4, 9], &[5, 8], &[], &[]),
            Err(CoverageStatsError::Queries(
                IntervalError::InvalidInterval { index: 1 }
            ))
        );
        for query in [false, true] {
            let result = if query {
                candidates::run(method, &[0], &[], &[], &[])
            } else {
                candidates::run(method, &[], &[], &[0], &[])
            };
            let inner = IntervalError::LengthMismatch([("starts", 1), ("ends", 0)]);
            assert_eq!(
                result,
                Err(if query {
                    CoverageStatsError::Queries(inner)
                } else {
                    CoverageStatsError::Intervals(inner)
                })
            );
        }
    }
}

#[test]
fn source_splitting_and_query_splitting_do_not_preserve_counts() {
    let original = check(&[(0, 10)], &[(1, 9)])[0];
    let split = check(&[(0, 10)], &[(1, 5), (5, 9)])[0];
    assert_eq!(split.overlap_count, original.overlap_count + 1);
    assert_eq!(split.covered_length, original.covered_length);
    let children = check(&[(0, 5), (5, 10)], &[(1, 9)]);
    assert_eq!(children.iter().map(|r| r.overlap_count).sum::<u64>(), 2);
    assert_eq!(
        children.iter().map(|r| r.covered_length).sum::<i128>(),
        original.covered_length
    );
}

#[test]
fn nonuniform_relabeling_changes_fraction_and_enlarging_can_reduce_fraction() {
    let a = check(&[(0, 4)], &[(0, 2)])[0];
    let b = check(&[(0, 10)], &[(0, 2)])[0];
    assert_eq!(a.overlap_count, b.overlap_count);
    assert!(a.covered_fraction > b.covered_fraction);
}

#[test]
fn prefix_lookup_before_after_inside_runs_and_gaps_is_exact() {
    let sources = [
        (0u64, 1u64 << 54),
        (3, 8),
        ((1u64 << 54) + 2, (1u64 << 54) + 5),
    ];
    let boundaries = [
        0,
        1,
        3,
        8,
        (1u64 << 54) - 1,
        1u64 << 54,
        (1u64 << 54) + 1,
        (1u64 << 54) + 2,
        (1u64 << 54) + 3,
        (1u64 << 54) + 5,
        (1u64 << 54) + 6,
    ];
    let (ss, se): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
    let expected = oracle::cells(&vec![0; boundaries.len()], &boundaries, &ss, &se);
    assert_eq!(
        candidates::prefixes(&ss, &se, &boundaries),
        expected
            .iter()
            .map(|r| r.covered_length)
            .collect::<Vec<_>>()
    );
}

#[test]
fn selector_boundaries_keep_exact_results_and_verify_both_query_orders() {
    for p in [31, 32, 33] {
        let sources: Vec<_> = (0..p).map(|i| (i * 4, i * 4 + 2)).collect();
        let queries: Vec<_> = (0..p).rev().map(|i| (i * 4 - 1, i * 4 + 3)).collect();
        check(&queries, &sources);
        // Starts are increasing, but ends decrease. The direct ordered scan
        // cannot be selected from start order alone.
        let nested: Vec<_> = (0..p).map(|i| (i, p * 5 - i)).collect();
        check(&nested, &sources);
        let ordered: Vec<_> = queries.iter().rev().copied().collect();
        check(&ordered, &sources);
        let mut with_empties = sources.clone();
        with_empties.extend([(0, 0), (9, 9)]);
        check(&queries, &with_empties);
        check(&ordered, &with_empties);
    }
}

fn rows(max: usize) -> impl Strategy<Value = Vec<(i64, i64)>> {
    prop::collection::vec(
        (-12i64..=12, -12i64..=12).prop_map(|(a, b)| (a.min(b), a.max(b))),
        0..=max,
    )
}

proptest! {
    #[test]
    fn prefix_lookup_matches_direct_cells_at_every_boundary(q in rows(18), s in rows(24)) {
        let (ss,se):(Vec<_>,Vec<_>)=s.iter().copied().unzip();
        let mut boundaries:Vec<_>=s.iter().chain(&q).flat_map(|&(a,b)|[a-1,a,a+1,b-1,b,b+1]).collect();
        boundaries.extend([-14,14]);
        boundaries.sort_unstable(); boundaries.dedup();
        let expected=oracle::cells(&vec![-14;boundaries.len()],&boundaries,&ss,&se);
        prop_assert_eq!(candidates::prefixes(&ss,&se,&boundaries),expected.iter().map(|r|r.covered_length).collect::<Vec<_>>());
    }

    #[test]
    fn larger_candidates_preserve_structural_and_metamorphic_invariants(q in rows(512), s in rows(768)) {
        // Sparse physical coordinates exceed f64's exact-integer range. No
        // quadratic cell oracle is used for these larger generated collections.
        let spread=|rows:Vec<(i64,i64)>|rows.into_iter().map(|(a,b)|((1i64<<54)+a*1_000_000_007,(1i64<<54)+b*1_000_000_007)).collect::<Vec<_>>();
        let q=spread(q); let s=spread(s);
        let (qs,qe):(Vec<_>,Vec<_>)=q.iter().copied().unzip();
        let (ss,se):(Vec<_>,Vec<_>)=s.iter().copied().unzip();
        let expected=solve(&q,&s);
        let nonempty=s.iter().filter(|&&(a,b)|a<b).count() as u64;
        for method in candidates::METHODS {
            let actual=candidates::run(method,&qs,&qe,&ss,&se).unwrap();
            prop_assert_eq!(&actual,&expected);
            for row in &actual {
                prop_assert!(row.overlap_count<=nonempty);
                prop_assert!(0<=row.covered_length && row.covered_length<=row.query_length);
                prop_assert_eq!(row.covered_fraction.is_none(),row.query_length==0);
                prop_assert_eq!(row.overlap_count==0,row.covered_length==0);
            }
            let mut ss=ss.clone(); let mut se=se.clone(); ss.reverse(); se.reverse();
            prop_assert_eq!(candidates::run(method,&qs,&qe,&ss,&se).unwrap(),actual.clone());
            ss.extend_from_within(..); se.extend_from_within(..);
            for (a,b) in actual.iter().zip(candidates::run(method,&qs,&qe,&ss,&se).unwrap()) {
                prop_assert_eq!(b.overlap_count,2*a.overlap_count);
                prop_assert_eq!(b.covered_length,a.covered_length);
                prop_assert_eq!(b.covered_fraction,a.covered_fraction);
            }
            let mut qs=qs.clone(); let mut qe=qe.clone(); qs.reverse(); qe.reverse();
            let mut reversed=actual; reversed.reverse();
            prop_assert_eq!(candidates::run(method,&qs,&qe,&ss[..ss.len()/2],&se[..se.len()/2]).unwrap(),reversed);
        }
    }

    #[test]
    fn arbitrary_signed_and_unsigned_coordinates(
        q in prop::collection::vec((any::<i64>(),any::<i64>()),0..=10),
        s in prop::collection::vec((any::<i64>(),any::<i64>()),0..=12),
        uq in prop::collection::vec((any::<u64>(),any::<u64>()),0..=10),
        us in prop::collection::vec((any::<u64>(),any::<u64>()),0..=12),
    ) {
        let q:Vec<_>=q.into_iter().map(|(a,b)|(a.min(b),a.max(b))).collect();
        let s:Vec<_>=s.into_iter().map(|(a,b)|(a.min(b),a.max(b))).collect();
        check(&q,&s);
        let uq:Vec<_>=uq.into_iter().map(|(a,b)|(a.min(b),a.max(b))).collect();
        let us:Vec<_>=us.into_iter().map(|(a,b)|(a.min(b),a.max(b))).collect();
        check(&uq,&us);
    }

    #[test]
    fn oracle_and_bitmap_agree(q in rows(18), s in rows(24)) {
        let out = check(&q,&s);
        let (qs,qe): (Vec<_>,Vec<_>) = q.iter().copied().unzip();
        let (ss,se): (Vec<_>,Vec<_>) = s.iter().copied().unzip();
        prop_assert_eq!(out.iter().map(|r| r.covered_length).collect::<Vec<_>>(), oracle::bitmap(&qs,&qe,&ss,&se));
    }

    #[test]
    fn permutations_duplication_empties_and_determinism(q in rows(18), s in rows(24), shift in 0usize..30) {
        let out = solve(&q,&s);
        let mut sources = s.clone(); sources.reverse();
        if !sources.is_empty() { let len = sources.len(); sources.rotate_left(shift % len); }
        prop_assert_eq!(&out, &solve(&q,&sources));
        sources.extend([(-20,-20),(0,0),(20,20)]);
        prop_assert_eq!(&out, &solve(&q,&sources));
        let mut queries = q.clone(); queries.reverse();
        let mut reversed = out.clone(); reversed.reverse();
        prop_assert_eq!(solve(&queries,&s), reversed);
        let doubled_queries: Vec<_> = q.iter().copied().chain(q.iter().copied()).collect();
        prop_assert_eq!(solve(&doubled_queries,&s), out.iter().copied().chain(out.iter().copied()).collect::<Vec<_>>());
        let doubled_sources: Vec<_> = s.iter().copied().chain(s.iter().copied()).collect();
        for (a,b) in out.iter().zip(solve(&q,&doubled_sources)) {
            prop_assert_eq!(b.overlap_count, 2*a.overlap_count);
            prop_assert_eq!(b.covered_length,a.covered_length);
            prop_assert_eq!(b.covered_fraction,a.covered_fraction);
        }
        prop_assert_eq!(&out,&solve(&q,&s));
    }

    #[test]
    fn affine_source_addition_partition_and_premerge(q in rows(16), s in rows(20), x in -15i64..15, y in -15i64..15, k in 1i64..8, d in -20i64..20) {
        let out = solve(&q,&s);
        let transform = |rows: &[(i64,i64)]| rows.iter().map(|&(a,b)| (k*a+d,k*b+d)).collect::<Vec<_>>();
        for (a,b) in out.iter().zip(solve(&transform(&q),&transform(&s))) {
            prop_assert_eq!(a.overlap_count,b.overlap_count);
            prop_assert_eq!(a.covered_length*i128::from(k),b.covered_length);
            prop_assert_eq!(a.query_length*i128::from(k),b.query_length);
            prop_assert_eq!(a.covered_fraction,b.covered_fraction);
        }
        let mut added = s.clone(); let extra = (x.min(y),x.max(y)); added.push(extra);
        for ((a,b), &(qs,qe)) in out.iter().zip(solve(&q,&added)).zip(&q) {
            let hit = qs < qe && extra.0 < extra.1 && extra.0 < qe && qs < extra.1;
            prop_assert_eq!(b.overlap_count,a.overlap_count+u64::from(hit));
            prop_assert!(b.covered_length>=a.covered_length);
            prop_assert!(b.covered_fraction>=a.covered_fraction);
        }
        let left = solve(&q,&s[..s.len()/2]); let right = solve(&q,&s[s.len()/2..]);
        for (((a,b),c),&(start,end)) in left.iter().zip(&right).zip(&out).zip(&q) {
            prop_assert_eq!(a.overlap_count+b.overlap_count,c.overlap_count);
            let shared_ticks=(start..end).filter(|&tick| {
                s[..s.len()/2].iter().any(|&(a,b)|a<=tick && tick<b)
                && s[s.len()/2..].iter().any(|&(a,b)|a<=tick && tick<b)
            }).count() as i128;
            prop_assert_eq!(c.covered_length,a.covered_length+b.covered_length-shared_ticks);
        }
        let (ss,se): (Vec<_>,Vec<_>) = s.iter().copied().unzip();
        for (a,b) in out.iter().zip(solve(&q,&merge_intervals(&ss,&se).unwrap())) {
            prop_assert_eq!(a.covered_length,b.covered_length);
            prop_assert_eq!(a.covered_fraction,b.covered_fraction);
            prop_assert!(b.overlap_count<=a.overlap_count);
        }
    }

    #[test]
    fn splitting_and_enlargement(q in rows(16), s in rows(20)) {
        let mut split_sources = Vec::new();
        for &(a,b) in &s { let t = a+(b-a)/2; split_sources.extend([(a,t),(t,b)]); }
        for ((original,split),&(a,b)) in solve(&q,&s).iter().zip(solve(&q,&split_sources)).zip(&q) {
            let both = s.iter().filter(|&&(u,v)| { let t = u+(v-u)/2; a<b && u<t && t<v && a<t && t<b }).count() as u64;
            prop_assert_eq!(split.overlap_count,original.overlap_count+both);
            prop_assert_eq!(split.covered_length,original.covered_length);
            prop_assert_eq!(split.covered_fraction,original.covered_fraction);
            let t = a+(b-a)/2;
            let children=solve(&[(a,t),(t,b)],&s);
            prop_assert_eq!(children[0].covered_length+children[1].covered_length,original.covered_length);
            prop_assert_eq!(children[0].query_length+children[1].query_length,original.query_length);
            let duplicated=s.iter().filter(|&&(u,v)| a<t && t<b && u<t && t<v).count() as u64;
            prop_assert_eq!(children[0].overlap_count+children[1].overlap_count,original.overlap_count+duplicated);
            let larger=solve(&[(a-1,b+1)],&s)[0];
            prop_assert!(larger.overlap_count>=original.overlap_count);
            prop_assert!(larger.covered_length>=original.covered_length);
        }
    }

    #[test]
    fn secondary_geometry_and_self_overlap_checks(q in rows(16), s in rows(20)) {
        let (ss,se): (Vec<_>,Vec<_>)=s.iter().copied().unzip();
        for (&(a,b),row) in q.iter().zip(solve(&q,&s)) {
            let measure=|runs: Vec<(i64,i64)>| runs.iter().map(|&(s,e)| i128::from(e)-i128::from(s)).sum::<i128>();
            prop_assert_eq!(measure(intersect_intervals(&[a],&[b],&ss,&se).unwrap()),row.covered_length);
            prop_assert_eq!(measure(subtract_intervals(&[a],&[b],&ss,&se).unwrap())+row.covered_length,row.query_length);
        }
        for ((row,count),&(a,b)) in solve(&s,&s).iter().zip(overlap_counts(&ss,&se).unwrap()).zip(&s) {
            prop_assert_eq!(row.overlap_count,if a<b { count as u64+1 } else { 0 });
        }
    }
}

#[test]
fn large_disjoint_and_one_union_run_have_structured_answers() {
    let sources: Vec<_> = (0..100_000i64).map(|i| (3 * i, 3 * i + 1)).collect();
    let queries: Vec<_> = (0..20_000i64).rev().map(|i| (3 * i, 3 * i + 4)).collect();
    for row in solve(&queries, &sources) {
        assert_eq!(
            (row.overlap_count, row.covered_length, row.query_length),
            (2, 2, 4)
        );
    }
    let sources = vec![(0i64, 100_000); 100_000];
    for row in solve(&queries, &sources) {
        assert_eq!((row.overlap_count, row.covered_length), (100_000, 4));
    }
}
