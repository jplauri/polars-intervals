use intervals_core::{
    CoverageSegment, IntervalError, assign_lanes, coverage_profile, max_weight_clique,
    weighted_coverage_profile,
};
use proptest::prelude::*;

type Row = (i64, i64, i128);
type Segment = CoverageSegment<i64>;

fn columns(rows: &[Row]) -> (Vec<i64>, Vec<i64>, Vec<i128>) {
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

fn solve(rows: &[Row], domain: Option<(i64, i64)>, zero: bool) -> Vec<Segment> {
    let (starts, ends, weights) = columns(rows);
    weighted_coverage_profile(&starts, &ends, &weights, domain, zero).unwrap()
}

fn segments(rows: &[Row]) -> Vec<Segment> {
    rows.iter()
        .map(|&(start, end, load)| CoverageSegment { start, end, load })
        .collect()
}

// Neither oracle uses signed events, sorted streams, or production helpers.
// Establish domain and cells from ORIGINAL rows, then evaluate the defining
// membership predicate for every row at each cell's inclusive left endpoint.
fn domain_of(rows: &[Row], domain: Option<(i64, i64)>) -> Option<(i64, i64)> {
    domain.or_else(|| {
        let nonempty: Vec<_> = rows.iter().filter(|r| r.0 < r.1).collect();
        Some((
            nonempty.iter().map(|r| r.0).min()?,
            nonempty.iter().map(|r| r.1).max()?,
        ))
    })
}

fn canonical_cells(cells: impl Iterator<Item = Row>, zero: bool) -> Vec<Segment> {
    let mut result: Vec<Segment> = Vec::new();
    for (start, end, load) in cells {
        if start >= end || (!zero && load == 0) {
            continue;
        }
        match result.last_mut() {
            Some(last) if last.end == start && last.load == load => last.end = end,
            _ => result.push(CoverageSegment { start, end, load }),
        }
    }
    result
}

fn oracle(rows: &[Row], domain: Option<(i64, i64)>, zero: bool) -> Vec<Segment> {
    let Some((left, right)) = domain_of(rows, domain) else {
        return vec![];
    };
    let mut boundaries = vec![left, right];
    for &(start, end, _) in rows {
        if start < end {
            boundaries.extend([start, end].into_iter().filter(|&t| left < t && t < right));
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    canonical_cells(
        boundaries.windows(2).map(|pair| {
            let load = rows
                .iter()
                .filter(|r| r.0 <= pair[0] && pair[0] < r.1)
                .map(|r| r.2)
                .sum();
            (pair[0], pair[1], load)
        }),
        zero,
    )
}

fn tick_oracle(rows: &[Row], domain: Option<(i64, i64)>, zero: bool) -> Vec<Segment> {
    let Some((left, right)) = domain_of(rows, domain) else {
        return vec![];
    };
    assert!(right - left <= 100, "tick oracle is only for small domains");
    canonical_cells(
        (left..right).map(|tick| {
            (
                tick,
                tick + 1,
                rows.iter()
                    .filter(|r| r.0 <= tick && tick < r.1)
                    .map(|r| r.2)
                    .sum(),
            )
        }),
        zero,
    )
}

fn invariants(rows: &[Row], result: &[Segment], domain: Option<(i64, i64)>, zero: bool) {
    let Some((left, right)) = domain_of(rows, domain) else {
        assert!(result.is_empty());
        return;
    };
    if left == right {
        assert!(result.is_empty());
        return;
    }
    let mut boundaries = vec![left, right];
    for &(s, e, _) in rows.iter().filter(|r| r.0 < r.1) {
        boundaries.extend([s, e].into_iter().filter(|&t| left <= t && t <= right));
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    assert!(result.len() < boundaries.len());
    for segment in result {
        assert!(left <= segment.start && segment.start < segment.end && segment.end <= right);
        assert!(segment.load >= 0 && (zero || segment.load > 0));
        assert!(boundaries.contains(&segment.start) && boundaries.contains(&segment.end));
    }
    for pair in result.windows(2) {
        assert!(pair[0].end <= pair[1].start);
        assert!(pair[0].end != pair[1].start || pair[0].load != pair[1].load);
        if zero {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }
    if zero {
        assert_eq!(result.first().unwrap().start, left);
        assert_eq!(result.last().unwrap().end, right);
    }
}

fn check(rows: &[Row], domain: Option<(i64, i64)>, zero: bool) -> Vec<Segment> {
    let actual = solve(rows, domain, zero);
    assert_eq!(actual, oracle(rows, domain, zero));
    invariants(rows, &actual, domain, zero);
    actual
}

#[test]
fn empty_singleton_duplicate_empties_and_all_zero_domains() {
    for zero in [false, true] {
        for rows in [vec![], vec![(4, 4, 9)], vec![(4, 4, 9), (4, 4, 0)]] {
            assert!(check(&rows, None, zero).is_empty());
            assert_eq!(
                check(&rows, Some((0, 8)), zero),
                if zero { segments(&[(0, 8, 0)]) } else { vec![] }
            );
            assert!(check(&rows, Some((2, 2)), zero).is_empty());
        }
        assert_eq!(check(&[(2, 5, 7)], None, zero), segments(&[(2, 5, 7)]));
        assert_eq!(
            check(&[(0, 3, 0), (8, 10, 0)], None, zero),
            if zero {
                segments(&[(0, 10, 0)])
            } else {
                vec![]
            }
        );
    }
    assert_eq!(coverage_profile::<i32>(&[], &[], None, true), Ok(vec![]));
    assert_eq!(coverage_profile(&[7, 7], &[7, 7], None, true), Ok(vec![]));
}

#[test]
fn documented_examples_units_weighted_and_membership_change() {
    let s = [0, 2, 5];
    let e = [4, 5, 7];
    assert_eq!(
        coverage_profile(&s, &e, None, false).unwrap(),
        segments(&[(0, 2, 1), (2, 4, 2), (4, 7, 1)])
    );
    assert_eq!(
        coverage_profile(&s, &e, None, false),
        weighted_coverage_profile(&s, &e, &[1, 1, 1], None, false)
    );
    let rows = [(0, 4, 2), (2, 5, 3), (5, 7, 3)];
    assert_eq!(
        check(&rows, None, false),
        segments(&[(0, 2, 2), (2, 4, 5), (4, 7, 3)])
    );
    assert_eq!(
        check(&rows, Some((-1, 8)), true),
        segments(&[(-1, 0, 0), (0, 2, 2), (2, 4, 5), (4, 7, 3), (7, 8, 0)])
    );
}

#[test]
fn overlap_touching_unequal_loads_and_zero_time_batches() {
    assert_eq!(
        check(&[(0, 3, 1), (1, 4, 1)], None, false),
        segments(&[(0, 1, 1), (1, 3, 2), (3, 4, 1)])
    );
    assert_eq!(
        check(&[(0, 2, 1), (2, 4, 1)], None, false),
        segments(&[(0, 4, 1)])
    );
    assert_eq!(
        check(&[(0, 2, 2), (2, 4, 3)], None, false),
        segments(&[(0, 2, 2), (2, 4, 3)])
    );
    assert_eq!(
        check(&[(0, 2, 2), (0, 2, 5), (2, 4, 3), (2, 4, 4)], None, false),
        segments(&[(0, 4, 7)])
    );
}

#[test]
fn equal_loads_never_coalesce_across_omitted_gaps() {
    let rows = [(0, 2, 3), (4, 6, 3)];
    assert_eq!(check(&rows, None, false), segments(&rows));
    assert_eq!(
        check(&rows, None, true),
        segments(&[(0, 2, 3), (2, 4, 0), (4, 6, 3)])
    );
    assert_eq!(
        check(&rows, Some((-2, 8)), true),
        segments(&[(-2, 0, 0), (0, 2, 3), (2, 4, 0), (4, 6, 3), (6, 8, 0)])
    );
    assert_eq!(
        check(&rows, Some((1, 5)), true),
        segments(&[(1, 2, 3), (2, 4, 0), (4, 5, 3)])
    );
}

#[test]
fn zero_weight_rows_establish_hull_but_empty_rows_do_not() {
    let rows = [
        (0, 100, 0),
        (10, 20, 5),
        (-90, -90, 8),
        (200, 200, i128::MAX),
    ];
    assert_eq!(
        check(&rows, None, true),
        segments(&[(0, 10, 0), (10, 20, 5), (20, 100, 0)])
    );
    assert_eq!(check(&rows, None, false), segments(&[(10, 20, 5)]));
}

#[test]
fn nesting_cliques_duplicates_chains_equal_starts_and_equal_ends() {
    for rows in [
        vec![(0, 10, 2), (1, 9, 3), (2, 8, 4), (3, 7, 5)],
        vec![(0, 6, 2), (1, 7, 3), (2, 8, 4), (3, 9, 5)],
        vec![(1, 5, 7); 12],
        (0..10).map(|i| (i, i + 2, 1)).collect(),
        (1..10).map(|i| (0, i, i128::from(i))).collect(),
        (0..9).map(|i| (i, 10, i128::from(i))).collect(),
        (0..30)
            .map(|i| (i % 4, i % 4 + 2, i128::from(i % 3)))
            .collect(),
    ] {
        for zero in [false, true] {
            check(&rows, None, zero);
        }
    }
}

#[test]
fn all_input_orderings_and_tail_expirations_match() {
    let rows = [(0, 20, 1), (1, 6, 2), (2, 17, 3), (3, 8, 4), (4, 12, 5)];
    let expected = check(&rows, None, true);
    assert_eq!(
        expected.last().unwrap(),
        &CoverageSegment {
            start: 17,
            end: 20,
            load: 1
        }
    );
    let mut reordered = rows;
    for order in [
        [4, 3, 2, 1, 0],
        [1, 3, 4, 2, 0],
        [2, 0, 4, 1, 3],
        [0, 1, 4, 3, 2],
    ] {
        for (i, j) in order.into_iter().enumerate() {
            reordered[i] = rows[j];
        }
        assert_eq!(check(&reordered, None, true), expected);
    }
}

#[test]
fn domain_before_after_inside_gap_endpoints_and_empty() {
    let rows = [(0, 4, 2), (8, 12, 3)];
    for bounds in [
        (-8, -2),
        (14, 18),
        (1, 3),
        (5, 7),
        (0, 12),
        (4, 8),
        (2, 10),
        (6, 6),
    ] {
        for zero in [false, true] {
            check(&rows, Some(bounds), zero);
        }
    }
}

#[test]
fn ordered_nonnumeric_and_extreme_endpoints_need_no_arithmetic() {
    assert_eq!(
        coverage_profile(&['a', 'c'], &['d', 'f'], None, false).unwrap(),
        vec![
            CoverageSegment {
                start: 'a',
                end: 'c',
                load: 1
            },
            CoverageSegment {
                start: 'c',
                end: 'd',
                load: 2
            },
            CoverageSegment {
                start: 'd',
                end: 'f',
                load: 1
            },
        ]
    );
    assert_eq!(
        coverage_profile(&[i64::MIN, 0], &[0, i64::MAX], None, true).unwrap(),
        segments(&[(i64::MIN, i64::MAX, 1)])
    );
    assert_eq!(
        coverage_profile(
            &[0u64, u64::MAX - 1],
            &[u64::MAX - 1, u64::MAX],
            None,
            false
        )
        .unwrap(),
        vec![CoverageSegment {
            start: 0,
            end: u64::MAX,
            load: 1
        }]
    );
    assert_eq!(
        coverage_profile(&[i8::MIN, 0], &[0, i8::MAX], None, true).unwrap(),
        vec![CoverageSegment {
            start: i8::MIN,
            end: i8::MAX,
            load: 1
        }]
    );
}

#[test]
fn exact_loads_above_u64_and_float_precision() {
    let large = u64::MAX;
    assert_eq!(
        weighted_coverage_profile(&[0, 0], &[2, 2], &[large, large], None, false).unwrap(),
        segments(&[(0, 2, 2 * i128::from(large))])
    );
    assert_eq!(
        check(&[(0, 3, (1i128 << 53) + 1), (1, 2, 1)], None, false),
        segments(&[
            (0, 1, (1i128 << 53) + 1),
            (1, 2, (1i128 << 53) + 2),
            (2, 3, (1i128 << 53) + 1)
        ])
    );
}

#[test]
fn overflow_only_for_represented_positive_length_load() {
    assert_eq!(
        weighted_coverage_profile(&[0, 1], &[3, 2], &[i128::MAX, 1], None, false),
        Err(IntervalError::LoadOverflow)
    );
    assert_eq!(
        check(&[(0, 2, i128::MAX), (2, 4, i128::MAX)], None, false),
        segments(&[(0, 4, i128::MAX)])
    );
    assert_eq!(
        check(&[(0, 2, i128::MAX), (4, 6, i128::MAX)], None, false),
        segments(&[(0, 2, i128::MAX), (4, 6, i128::MAX)])
    );
    assert_eq!(
        check(
            &[(0, 4, 2), (2, 2, i128::MAX), (2, 2, i128::MAX)],
            None,
            false
        ),
        segments(&[(0, 4, 2)])
    );
    // Weighted area overflows, but constructing the profile does not integrate it.
    assert_eq!(
        check(&[(i64::MIN, i64::MAX, i128::MAX)], None, false),
        segments(&[(i64::MIN, i64::MAX, i128::MAX)])
    );
    let s = [0, 1];
    let e = [4, 2];
    let w = [i128::MAX, 1];
    let rows = [(0, 4, i128::MAX), (1, 2, 1)];
    for bounds in [(0, 1), (2, 4), (1, 1), (4, 8), (-4, 0)] {
        for zero in [false, true] {
            check(&rows, Some(bounds), zero);
        }
    }
    assert_eq!(
        weighted_coverage_profile(&s, &e, &w, Some((1, 2)), true),
        Err(IntervalError::LoadOverflow)
    );
}

#[test]
fn rejects_lengths_domains_and_irrelevant_invalid_original_rows() {
    assert_eq!(
        coverage_profile(&[0], &[], None, false),
        Err(IntervalError::LengthMismatch([("starts", 1), ("ends", 0)]))
    );
    assert_eq!(
        weighted_coverage_profile(&[0], &[1], &[] as &[i64], None, false),
        Err(IntervalError::LengthMismatch([
            ("intervals", 1),
            ("weights", 0)
        ]))
    );
    assert_eq!(
        coverage_profile::<i32>(&[], &[], Some((2, 1)), false),
        Err(IntervalError::InvalidDomain)
    );
    for domain in [None, Some((0, 0)), Some((10, 12))] {
        for weights in [[0i128, 0, 0], [1, 0, i128::MAX]] {
            assert_eq!(
                weighted_coverage_profile(&[0, 4, 3], &[0, 2, 3], &weights, domain, false),
                Err(IntervalError::InvalidInterval { index: 1 })
            );
        }
        for (s, e) in [([0, 5], [0, 5]), ([0, 5], [2, 7])] {
            assert_eq!(
                weighted_coverage_profile(&s, &e, &[0, -1], domain, true),
                Err(IntervalError::NegativeLoad { index: 1 })
            );
        }
        assert_eq!(
            coverage_profile(&[0, 9], &[0, 1], domain, false),
            Err(IntervalError::InvalidInterval { index: 1 })
        );
    }
    assert_eq!(
        IntervalError::NegativeLoad { index: 7 }.to_string(),
        "load at index 7 is negative; loads must be nonnegative"
    );
    assert_eq!(
        IntervalError::LoadOverflow.to_string(),
        "coverage load exceeds the i128 accumulator range"
    );
}

fn instances(maximum: usize) -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec((-10i64..=10, -10i64..=10, 0i128..=9), 0..=maximum).prop_map(|rows| {
        rows.into_iter()
            .map(|(a, b, q)| (a.min(b), a.max(b), q))
            .collect()
    })
}

fn domains() -> impl Strategy<Value = Option<(i64, i64)>> {
    prop::option::of((-15i64..=15, -15i64..=15).prop_map(|(a, b)| (a.min(b), a.max(b))))
}

fn at(profile: &[Segment], t: i64) -> i128 {
    profile
        .iter()
        .find(|s| s.start <= t && t < s.end)
        .map_or(0, |s| s.load)
}

// Default configuration honors PROPTEST_CASES. For a deeper local pass:
// PROPTEST_CASES=2048 cargo test -p intervals-core --test coverage_profile --locked
// Standard source-parallel persistence retains any minimized regressions.
proptest! {
    #[test]
    fn entire_output_matches_membership_and_tick_oracles(rows in instances(35), domain in domains(), zero in any::<bool>()) {
        let actual = check(&rows, domain, zero);
        prop_assert_eq!(actual, tick_oracle(&rows, domain, zero));
        let (s, e, _) = columns(&rows);
        let units: Vec<_> = rows.iter().map(|r| (r.0, r.1, 1)).collect();
        let unit_result = coverage_profile(&s, &e, domain, zero).unwrap();
        prop_assert_eq!(&unit_result, &check(&units, domain, zero));
        prop_assert_eq!(unit_result, tick_oracle(&units, domain, zero));
    }

    #[test]
    fn larger_instances_are_structural_and_permutation_invariant(rows in instances(250), domain in domains(), zero in any::<bool>(), rotation in any::<usize>()) {
        let expected = solve(&rows, domain, zero);
        invariants(&rows, &expected, domain, zero);
        let mut shuffled = rows;
        shuffled.reverse();
        if !shuffled.is_empty() { let shift = rotation % shuffled.len(); shuffled.rotate_left(shift); }
        prop_assert_eq!(&solve(&shuffled, domain, zero), &expected);
        shuffled.sort_by_key(|r| r.1);
        prop_assert_eq!(solve(&shuffled, domain, zero), expected);
    }

    #[test]
    fn increasing_nonlinear_relabeling_and_translation(rows in instances(40), domain in domains(), zero in any::<bool>(), delta in -100i64..=100) {
        let expected = solve(&rows, domain, zero);
        // Strictly increasing on all integers, changes coordinate distances.
        let relabel = |t: i64| t * t * t + 2 * t;
        for map in [&relabel as &dyn Fn(i64) -> i64, &|t| t + delta] {
            let changed: Vec<_> = rows.iter().map(|r| (map(r.0), map(r.1), r.2)).collect();
            let bounds = domain.map(|(a,b)| (map(a),map(b)));
            let transformed: Vec<_> = expected.iter().map(|s| CoverageSegment { start: map(s.start), end: map(s.end), load: s.load }).collect();
            prop_assert_eq!(solve(&changed, bounds, zero), transformed);
        }
    }

    #[test]
    fn positive_scaling_and_duplication(rows in instances(40), domain in domains(), zero in any::<bool>(), factor in 1i128..=10) {
        let expected = solve(&rows, domain, zero);
        let scaled: Vec<_> = rows.iter().map(|r| (r.0,r.1,r.2 * factor)).collect();
        let scale_profile = |factor| expected.iter().map(|s| CoverageSegment { load: s.load * factor, ..*s }).collect::<Vec<_>>();
        prop_assert_eq!(solve(&scaled, domain, zero), scale_profile(factor));
        let doubled: Vec<_> = rows.iter().chain(&rows).copied().collect();
        prop_assert_eq!(solve(&doubled, domain, zero), scale_profile(2));
    }

    #[test]
    fn splitting_rows_and_empty_additions_preserve_canonical_output(rows in instances(35), domain in domains(), zero in any::<bool>(), t in -100i64..=100) {
        let expected = solve(&rows, domain, zero);
        let mut split = Vec::new();
        for &(a,c,q) in &rows {
            if c - a > 1 { let b = a + (c-a)/2; split.extend([(a,b,q),(b,c,q)]); }
            else { split.push((a,c,q)); }
        }
        split.extend([(t,t,i128::MAX),(t,t,0)]);
        prop_assert_eq!(solve(&split, domain, zero), expected);
    }

    #[test]
    fn zero_additions_preserve_function_but_can_extend_inferred_domain(rows in instances(30), zero in any::<bool>()) {
        let original = solve(&rows, None, zero);
        let mut extended = rows.clone(); extended.push((-20,20,0));
        let actual = check(&extended, None, zero);
        for t in -20..20 { prop_assert_eq!(at(&original,t), at(&actual,t)); }
        prop_assert_eq!(solve(&rows, Some((-25,25)), zero), solve(&extended, Some((-25,25)), zero));
        if zero { prop_assert_eq!(actual.first().unwrap().start,-20); prop_assert_eq!(actual.last().unwrap().end,20); }
    }

    #[test]
    fn arbitrary_partition_profiles_add_pointwise(rows in instances(35), selectors in prop::collection::vec(any::<bool>(), 0..=35), a in -15i64..=15, b in -15i64..=15) {
        let domain = Some((a.min(b),a.max(b)));
        let mut left = Vec::new(); let mut right = Vec::new();
        for (i,&row) in rows.iter().enumerate() {
            if selectors.get(i).copied().unwrap_or(false) { left.push(row); } else { right.push(row); }
        }
        let full = solve(&rows,domain,true);
        let lp = solve(&left,domain,true); let rp = solve(&right,domain,true);
        let mut points: Vec<_> = lp.iter().chain(&rp).flat_map(|s| [s.start,s.end]).collect();
        points.sort_unstable(); points.dedup();
        let combined = canonical_cells(points.windows(2).map(|p| (p[0],p[1],at(&lp,p[0])+at(&rp,p[0]))),true);
        prop_assert_eq!(combined,full);
    }

    #[test]
    fn profile_idempotence_in_wide_weight_core(rows in instances(40), domain in domains(), zero in any::<bool>()) {
        let result = solve(&rows,domain,zero);
        let as_rows: Vec<_> = result.iter().map(|s| (s.start,s.end,s.load)).collect();
        prop_assert_eq!(solve(&as_rows,domain,zero),result);
    }

    #[test]
    fn sparse_is_positive_filter_and_restriction_agrees(rows in instances(40), a in -15i64..=15, b in -15i64..=15) {
        let outer = Some((-15,15));
        let full = solve(&rows,outer,true);
        let filtered: Vec<_> = full.iter().filter(|s| s.load>0).copied().collect();
        prop_assert_eq!(solve(&rows,outer,false),filtered);
        let smaller = Some((a.min(b),a.max(b)));
        let as_rows: Vec<_> = full.iter().map(|s| (s.start,s.end,s.load)).collect();
        for zero in [false,true] { prop_assert_eq!(solve(&rows,smaller,zero),solve(&as_rows,smaller,zero)); }
    }

    #[test]
    fn bounded_area_is_conserved(rows in instances(60), domain in domains(), zero in any::<bool>()) {
        let result = solve(&rows,domain,zero);
        let output_area: i128 = result.iter().map(|s| s.load * (i128::from(s.end)-i128::from(s.start))).sum();
        let input_area: i128 = domain_of(&rows,domain).map_or(0,|(left,right)| rows.iter().map(|&(a,b,q)| q*(i128::from(b.min(right))-i128::from(a.max(left))).max(0)).sum());
        prop_assert_eq!(output_area,input_area);
    }

    #[test]
    fn nonnegative_addition_cannot_lower_load(rows in instances(35), a in -15i64..=15, b in -15i64..=15, q in 0i128..=20) {
        let domain = Some((-15,15));
        let original = solve(&rows,domain,true);
        let mut more = rows; more.push((a.min(b),a.max(b),q));
        let added = solve(&more,domain,true);
        for t in -15..15 { prop_assert!(at(&added,t)>=at(&original,t)); }
    }

    #[test]
    fn profile_peak_matches_clique_weight_and_unit_lane_count(rows in instances(35), domain in domains()) {
        let Some((left,right)) = domain_of(&rows,domain) else { return Ok(()); };
        let clipped: Vec<_> = rows.iter().map(|&(a,b,q)| (a.max(left),b.min(right),q)).filter(|r| r.0<r.1).collect();
        let (s,e,w) = columns(&clipped);
        let selected = max_weight_clique(&s,&e,&w).unwrap();
        let clique: i128 = w.iter().zip(&selected).filter(|(_,keep)| **keep).map(|(&q,_)| q).sum();
        prop_assert_eq!(solve(&rows,domain,false).iter().map(|s|s.load).max().unwrap_or(0),clique);
        let lanes = assign_lanes(&s,&e).unwrap().into_iter().max().map_or(0,|i| i128::from(i)+1);
        let (s,e,_) = columns(&rows);
        prop_assert_eq!(coverage_profile(&s,&e,domain,false).unwrap().iter().map(|s|s.load).max().unwrap_or(0),lanes);
    }

    #[test]
    fn arbitrary_wide_ordered_endpoints_match_oracle(raw in prop::collection::vec((any::<i64>(),any::<i64>(),0i128..=9),0..=15), bounds in prop::option::of((any::<i64>(),any::<i64>())), zero in any::<bool>()) {
        let rows: Vec<_> = raw.into_iter().map(|(a,b,q)| (a.min(b),a.max(b),q)).collect();
        check(&rows,bounds.map(|(a,b)| (a.min(b),a.max(b))),zero);
    }
}
