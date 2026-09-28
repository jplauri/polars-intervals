#[path = "../benches/support/mod.rs"]
mod support;

use intervals_core::{
    BalanceStopReason, IntervalError, assign_balanced_lanes,
    assign_balanced_lanes_with_diagnostics, assign_lanes,
};
use proptest::prelude::*;

fn score(lanes: &[u32]) -> (usize, u128) {
    let k = lanes.iter().max().map_or(0, |&v| v as usize + 1);
    let mut loads = vec![0usize; k];
    for &lane in lanes {
        loads[lane as usize] += 1;
    }
    (
        loads.iter().max().unwrap_or(&0) - loads.iter().min().unwrap_or(&0),
        loads.iter().map(|&v| (v as u128).pow(2)).sum(),
    )
}

fn overlaps(starts: &[i64], ends: &[i64], i: usize, j: usize) -> bool {
    starts[i] < ends[i] && starts[j] < ends[j] && starts[i] < ends[j] && starts[j] < ends[i]
}

fn verify(starts: &[i64], ends: &[i64], lanes: &[u32]) {
    support::verify(starts, ends, lanes, support::optimum(starts, ends));
    for i in 0..starts.len() {
        for j in 0..i {
            assert!(!overlaps(starts, ends, i, j) || lanes[i] != lanes[j]);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Oracle {
    Exact((usize, u128)),
    Unknown,
}

// Deliberately simple independent coloring enumeration. A hard n cap makes
// "exact" a proven result, never an interrupted search's incumbent.
fn oracle(starts: &[i64], ends: &[i64]) -> Oracle {
    if starts.len() > 9 {
        return Oracle::Unknown;
    }
    let k = support::optimum(starts, ends);
    fn visit(
        starts: &[i64],
        ends: &[i64],
        k: usize,
        lanes: &mut Vec<u32>,
        used: usize,
        best: &mut (usize, u128),
    ) {
        let row = lanes.len();
        if row == starts.len() {
            if used == k {
                *best = (*best).min(score(lanes));
            }
            return;
        }
        for lane in 0..(used + 1).min(k) {
            if (0..row)
                .any(|other| lanes[other] as usize == lane && overlaps(starts, ends, row, other))
            {
                continue;
            }
            lanes.push(lane as u32);
            visit(starts, ends, k, lanes, used.max(lane + 1), best);
            lanes.pop();
        }
    }
    let mut best = (usize::MAX, u128::MAX);
    visit(starts, ends, k, &mut Vec::new(), 0, &mut best);
    Oracle::Exact(best)
}

// Independently discovers pair components by O(n²) adjacency/BFS, then enumerates
// every orientation. It intentionally shares neither sweep nor DP production code.
fn pair_optimum(starts: &[i64], ends: &[i64], lanes: &[u32], a: u32, b: u32) -> usize {
    let mut seen = vec![false; lanes.len()];
    let mut sides = Vec::new();
    for root in 0..lanes.len() {
        if seen[root] || (lanes[root] != a && lanes[root] != b) {
            continue;
        }
        seen[root] = true;
        let mut queue = vec![root];
        let mut counts = (0usize, 0usize);
        while let Some(row) = queue.pop() {
            if lanes[row] == a {
                counts.0 += 1;
            } else {
                counts.1 += 1;
            }
            for next in 0..lanes.len() {
                if !seen[next]
                    && (lanes[next] == a || lanes[next] == b)
                    && overlaps(starts, ends, row, next)
                {
                    seen[next] = true;
                    queue.push(next);
                }
            }
        }
        sides.push(counts);
    }
    assert!(sides.len() <= 20, "orientation oracle must stay tiny");
    (0..1usize << sides.len())
        .map(|mask| {
            let (mut x, mut y) = (0usize, 0usize);
            for (j, &(a, b)) in sides.iter().enumerate() {
                if mask & (1 << j) == 0 {
                    x += a;
                    y += b;
                } else {
                    x += b;
                    y += a;
                }
            }
            x.abs_diff(y)
        })
        .min()
        .unwrap()
}

fn check(starts: &[i64], ends: &[i64]) {
    let baseline = assign_lanes(starts, ends).unwrap();
    for budget in [0, 1, 200, 100_000] {
        let constructed =
            assign_balanced_lanes_with_diagnostics(starts, ends, None, budget).unwrap();
        let repaired =
            assign_balanced_lanes_with_diagnostics(starts, ends, Some(&baseline), budget).unwrap();
        for result in [&constructed, &repaired] {
            verify(starts, ends, &result.lanes);
            assert!(score(&result.lanes) <= score(&baseline));
            assert!(result.diagnostics.work <= budget);
        }
        assert_eq!(
            constructed,
            assign_balanced_lanes_with_diagnostics(starts, ends, None, budget).unwrap()
        );
        assert_eq!(
            repaired,
            assign_balanced_lanes_with_diagnostics(starts, ends, Some(&baseline), budget).unwrap()
        );
        if budget == 0 {
            assert_eq!(constructed.lanes, baseline);
            assert_eq!(repaired.lanes, baseline);
        }
    }
}

#[test]
fn deterministic_families_and_order_only_endpoints() {
    for (s, e) in [
        (vec![], vec![]),
        (vec![1], vec![2]),
        (vec![1; 12], vec![1; 12]),
        (vec![0, 1, 2], vec![1, 2, 3]),
        (vec![0; 8], vec![10; 8]),
        (vec![3, 0, 2, 1], vec![7, 10, 8, 9]),
        (vec![0, 0, 2, 4, 1, 2, 2], vec![4, 0, 2, 4, 3, 2, 2]),
        (
            vec![i64::MIN, 0, i64::MAX],
            vec![i64::MAX, i64::MAX, i64::MAX],
        ),
    ] {
        check(&s, &e);
    }
    let s = ['a', 'a', 'b', 'c', 'd'];
    let e = ['z', 'b', 'c', 'd', 'd'];
    for budget in [0, 100_000] {
        assert_eq!(
            assign_balanced_lanes(&s, &e, None, budget).unwrap().len(),
            5
        );
        assert_eq!(
            assign_balanced_lanes(
                &[0u64, u64::MAX - 1, u64::MAX],
                &[u64::MAX; 3],
                None,
                budget
            )
            .unwrap()
            .len(),
            3
        );
    }
}

#[test]
fn validates_before_all_early_returns() {
    for budget in [0, 1, 100_000] {
        assert!(matches!(
            assign_balanced_lanes(&[1], &[], None, budget),
            Err(IntervalError::LengthMismatch { .. })
        ));
        assert_eq!(
            assign_balanced_lanes(&[9, 0, 5, 3], &[10, 0, 4, 2], None, budget),
            Err(IntervalError::InvalidInterval { index: 2 })
        );
        assert!(matches!(
            assign_balanced_lanes::<i32>(&[], &[], Some(&[0]), budget),
            Err(IntervalError::LaneLengthMismatch { .. })
        ));
        assert!(matches!(
            assign_balanced_lanes(&[0], &[0], Some(&[u32::MAX]), budget),
            Err(IntervalError::InvalidLaneId { .. })
        ));
        assert!(matches!(
            assign_balanced_lanes(&[0, 0, 1], &[1, 1, 2], Some(&[0, 2, 0]), budget),
            Err(IntervalError::InvalidLaneId { .. })
        ));
        assert!(matches!(
            assign_balanced_lanes(&[0, 0], &[2, 2], Some(&[0, 0]), budget),
            Err(IntervalError::LaneConflict { .. })
        ));
        assert_eq!(
            assign_balanced_lanes(&[0, 1], &[1, 2], Some(&[0, 1]), budget),
            Err(IntervalError::NonMinimumLanes {
                actual: 2,
                minimum: 1
            })
        );
        assert_eq!(
            assign_balanced_lanes(&[0, 0], &[0, 0], Some(&[0, 1]), budget),
            Err(IntervalError::NonMinimumLanes {
                actual: 2,
                minimum: 1
            })
        );
        assert!(matches!(
            assign_balanced_lanes(&[2], &[1], Some(&[0]), budget),
            Err(IntervalError::InvalidInterval { index: 0 })
        ));
    }
}

fn stars(leaves: &[i64], larger_lanes: &[u32]) -> (Vec<i64>, Vec<i64>, Vec<u32>) {
    let (mut s, mut e, mut lanes) = (Vec::new(), Vec::new(), Vec::new());
    for (j, (&leaves, &lane)) in leaves.iter().zip(larger_lanes).enumerate() {
        let offset = j as i64 * 100;
        s.push(offset);
        e.push(offset + leaves * 2 + 2);
        lanes.push(1 - lane);
        for leaf in 0..leaves {
            s.push(offset + leaf * 2 + 1);
            e.push(offset + leaf * 2 + 2);
            lanes.push(lane);
        }
    }
    (s, e, lanes)
}

#[test]
fn simultaneous_flips_escape_single_component_trap() {
    let (s, e, lanes) = stars(&[9, 8, 7, 6], &[0, 0, 1, 1]);
    assert_eq!(score(&lanes), (4, 19 * 19 + 15 * 15));
    for weight in [8usize, 7, 6, 5] {
        assert!((4isize - 2 * weight as isize).unsigned_abs() >= 4);
    }
    let result = assign_balanced_lanes_with_diagnostics(&s, &e, Some(&lanes), 100_000).unwrap();
    verify(&s, &e, &result.lanes);
    assert_eq!(score(&result.lanes), (0, 2 * 17 * 17));
    assert_eq!(result.diagnostics.stop_reason, BalanceStopReason::Equity);
    assert!(result.diagnostics.flips >= 2);
}

#[test]
fn empty_rows_count_and_move_without_new_lanes() {
    let s = [0, 0, 5, 5, 5, 5];
    let e = [10, 10, 5, 5, 5, 5];
    let baseline = assign_lanes(&s, &e).unwrap();
    assert_eq!(&baseline[2..], &[0; 4]);
    for result in [
        assign_balanced_lanes(&s, &e, Some(&baseline), 100_000).unwrap(),
        assign_balanced_lanes(&s, &e, None, 100_000).unwrap(),
    ] {
        verify(&s, &e, &result);
        assert_eq!(score(&result).0, 0);
    }
}

#[test]
fn local_optimum_is_not_global_optimum() {
    let weights = [9, 6, 5, 5, 4, 1];
    let extras = [0u32, 2, 1, 2, 1, 1];
    let equitable_extras = [0u32, 1, 2, 2, 1, 0];
    let (mut s, mut e, mut lanes, mut equitable) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (j, &weight) in weights.iter().enumerate() {
        let offset = j as i64 * 100;
        for color in 0..3 {
            if color != extras[j] {
                s.push(offset);
                e.push(offset + 50);
                lanes.push(color);
            }
        }
        for color in 0..3 {
            if color != equitable_extras[j] {
                equitable.push(color);
            }
        }
        for leaf in 0..=weight {
            s.push(offset + 2 * leaf + 1);
            e.push(offset + 2 * leaf + 2);
            lanes.push(extras[j]);
            equitable.push(equitable_extras[j]);
        }
    }
    assert_eq!(score(&lanes), (2, 15 * 15 + 16 * 16 + 17 * 17));
    verify(&s, &e, &equitable);
    assert_eq!(score(&equitable), (0, 3 * 16 * 16));
    let result = assign_balanced_lanes_with_diagnostics(&s, &e, Some(&lanes), 100_000).unwrap();
    assert_eq!(result.lanes, lanes);
    assert_eq!(
        result.diagnostics.stop_reason,
        BalanceStopReason::PairwiseFixedPoint
    );
    for a in 0..3 {
        for b in a + 1..3 {
            let old = lanes
                .iter()
                .filter(|&&v| v == a)
                .count()
                .abs_diff(lanes.iter().filter(|&&v| v == b).count());
            assert_eq!(pair_optimum(&s, &e, &lanes, a, b), old);
        }
    }
    assert_eq!(oracle(&s, &e), Oracle::Unknown);
}

#[test]
fn atomic_budget_skips_and_high_k_stop() {
    let (s, e, lanes) = stars(&[9, 8, 7, 6], &[0, 0, 1, 1]);
    let result = assign_balanced_lanes_with_diagnostics(&s, &e, Some(&lanes), 50).unwrap();
    assert_eq!(result.lanes, lanes);
    assert_eq!(
        result.diagnostics.stop_reason,
        BalanceStopReason::BudgetExhausted
    );
    assert_eq!(result.diagnostics.skips, 1);
    let result =
        assign_balanced_lanes_with_diagnostics(&vec![0; 10_000], &vec![1; 10_000], None, 1)
            .unwrap();
    assert_eq!(result.diagnostics.stop_reason, BalanceStopReason::Equity);
    assert_eq!(result.diagnostics.work, 0);
    let mut s = vec![0; 1_000];
    let mut e = vec![1; 1_000];
    s.extend([2; 1_000]);
    e.extend([2; 1_000]);
    let baseline = assign_lanes(&s, &e).unwrap();
    let result = assign_balanced_lanes_with_diagnostics(&s, &e, Some(&baseline), 20_000).unwrap();
    assert!(result.diagnostics.work <= 20_000);
    assert_eq!(
        result.diagnostics.stop_reason,
        BalanceStopReason::BudgetExhausted
    );
}

fn valid(max: usize) -> impl Strategy<Value = (Vec<i64>, Vec<i64>)> {
    prop::collection::vec((-8i64..=8, 0i64..=8), 0..=max)
        .prop_map(|rows| rows.into_iter().map(|(s, d)| (s, s + d)).unzip())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn structure_nonregression_budget_and_determinism((s,e) in valid(30)) { check(&s,&e); }
    #[test]
    fn increasing_representations_and_permutations((s,e) in valid(25), offset in -100i64..100, scale in 1i64..10, seed in 1u64..=u64::MAX) {
        let reference = assign_balanced_lanes(&s, &e, None, 100_000).unwrap();
        let scaled_s: Vec<_> = s.iter().map(|v| v * scale + offset).collect();
        let scaled_e: Vec<_> = e.iter().map(|v| v * scale + offset).collect();
        prop_assert_eq!(&reference, &assign_balanced_lanes(&scaled_s, &scaled_e, None, 100_000).unwrap());
        let initial = assign_lanes(&s,&e).unwrap();
        let repaired = assign_balanced_lanes(&s, &e, Some(&initial), 100_000).unwrap();
        prop_assert_eq!(&repaired, &assign_balanced_lanes(&scaled_s, &scaled_e, Some(&initial), 100_000).unwrap());
        let mut coordinates: Vec<_> = s.iter().chain(&e).copied().collect(); coordinates.sort_unstable(); coordinates.dedup();
        let ranked_s: Vec<_> = s.iter().map(|v| coordinates.binary_search(v).unwrap()).collect();
        let ranked_e: Vec<_> = e.iter().map(|v| coordinates.binary_search(v).unwrap()).collect();
        prop_assert_eq!(&reference, &assign_balanced_lanes(&ranked_s, &ranked_e, None, 100_000).unwrap());
        prop_assert_eq!(&repaired, &assign_balanced_lanes(&ranked_s, &ranked_e, Some(&initial), 100_000).unwrap());
        let mut rows: Vec<_> = s.iter().copied().zip(e.iter().copied()).collect(); support::shuffle(&mut rows,&mut seed.clone());
        let (ps,pe): (Vec<_>,Vec<_>) = rows.into_iter().unzip();
        verify(&ps,&pe,&assign_balanced_lanes(&ps, &pe, None, 100_000).unwrap());
    }
    #[test]
    fn adding_empty_and_repairing_other_valid_colorings((mut s,mut e) in valid(20), x in -10i64..10) {
        let old_k = support::optimum(&s,&e); s.extend([x,x]); e.extend([x,x]);
        let k = support::optimum(&s,&e);
        prop_assert_eq!(k,old_k.max(1));
        let relabeled = assign_lanes(&s,&e).unwrap().iter().map(|&lane| k as u32 - 1 - lane).collect();
        for initial in [relabeled, assign_balanced_lanes(&s, &e, None, 100_000).unwrap()] {
            let result = assign_balanced_lanes(&s, &e, Some(&initial), 100_000).unwrap(); verify(&s,&e,&result);
            prop_assert!(score(&result) <= score(&initial));
            prop_assert_eq!(assign_balanced_lanes(&s, &e, Some(&initial), 0).unwrap(),initial);
        }
    }
    #[test]
    fn invalid_ids_are_rejected_at_zero_budget((s,e) in valid(20)) {
        let mut lanes = assign_lanes(&s,&e).unwrap();
        if !lanes.is_empty() { lanes[0] = u32::MAX; prop_assert!(assign_balanced_lanes(&s, &e, Some(&lanes), 0).is_err()); }
        lanes.push(0); prop_assert!(assign_balanced_lanes(&s, &e, Some(&lanes), 0).is_err());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]
    #[test]
    fn reconstructed_two_lane_coloring_matches_orientation_enumeration(
        components in prop::collection::vec((1i64..=9, any::<bool>()), 1..=8),
    ) {
        let leaves: Vec<_> = components.iter().map(|&(n,_)| n).collect();
        let labels: Vec<_> = components.iter().map(|&(_,lane)| u32::from(lane)).collect();
        let (s,e,initial) = stars(&leaves,&labels);
        let expected = pair_optimum(&s,&e,&initial,0,1);
        let result = assign_balanced_lanes(&s, &e, Some(&initial), 1_000_000).unwrap();
        verify(&s,&e,&result);
        prop_assert_eq!(score(&result).0,expected);
    }
    #[test]
    fn tiny_global_and_pair_oracles((s,e) in valid(8)) {
        let k = support::optimum(&s,&e);
        let baseline = assign_lanes(&s,&e).unwrap();
        let result = assign_balanced_lanes_with_diagnostics(&s, &e, Some(&baseline), 1_000_000).unwrap();
        let Oracle::Exact(optimum) = oracle(&s,&e) else { panic!("tiny input exceeded oracle cap") };
        prop_assert!(score(&result.lanes) >= optimum);
        if k <= 2 { prop_assert_eq!(score(&result.lanes),optimum); }
        if result.diagnostics.stop_reason == BalanceStopReason::PairwiseFixedPoint {
            for a in 0..k as u32 { for b in a + 1..k as u32 {
                let difference = result.lanes.iter().filter(|&&v| v == a).count().abs_diff(result.lanes.iter().filter(|&&v| v == b).count());
                prop_assert_eq!(pair_optimum(&s,&e,&result.lanes,a,b),difference);
            }}
        }
    }
}
