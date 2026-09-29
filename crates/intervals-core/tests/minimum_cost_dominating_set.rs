use intervals_core::{IntervalError, minimum_cost_dominating_set, minimum_dominating_set};
use proptest::prelude::*;

#[path = "../benches/support/domination.rs"]
mod candidates;

type Objective = (i128, usize);

// This oracle works on the ORIGINAL graph. In particular, self-domination
// never goes through overlap, so even duplicate empty rows remain mandatory.
fn dominates<T: Ord>(s: &[T], e: &[T], selected: &[bool]) -> bool {
    (0..s.len()).all(|i| {
        selected[i]
            || (0..s.len())
                .any(|j| selected[j] && s[i] < e[i] && s[j] < e[j] && s[i] < e[j] && s[j] < e[i])
    })
}

fn objective(w: &[i128], selected: &[bool]) -> Objective {
    w.iter()
        .zip(selected)
        .filter(|(_, selected)| **selected)
        .fold((0i128, 0), |(cost, count), (&w, _)| {
            (cost.checked_add(w).unwrap(), count + 1)
        })
}

fn brute_force(s: &[i64], e: &[i64], w: &[i128]) -> (Objective, usize, Vec<bool>) {
    let mut best = None;
    let mut ways = 0;
    let mut winner = Vec::new();
    for bits in 0usize..1 << s.len() {
        let mask: Vec<_> = (0..s.len()).map(|i| bits & (1 << i) != 0).collect();
        if !dominates(s, e, &mask) {
            continue;
        }
        let value = objective(w, &mask);
        if best.is_none_or(|old| value < old) {
            best = Some(value);
            winner = mask;
            ways = 1;
        } else if best == Some(value) {
            ways += 1;
        }
    }
    (best.unwrap(), ways, winner)
}

fn verify(s: &[i64], e: &[i64], w: &[i128], mask: &[bool], expected: Objective) {
    assert_eq!(mask.len(), s.len());
    assert!(dominates(s, e, mask), "infeasible mask {mask:?}");
    assert_eq!(objective(w, mask), expected);
    for selected in (0..s.len()).filter(|&i| mask[i]) {
        let mut removed = mask.to_vec();
        removed[selected] = false;
        assert!(
            !dominates(s, e, &removed),
            "selected row {selected} has no private domination witness"
        );
    }
}

fn check(s: &[i64], e: &[i64], w: &[i128]) -> (Vec<bool>, Objective) {
    let (expected, ways, winner) = brute_force(s, e, w);
    let result = minimum_cost_dominating_set(s, e, w).unwrap();
    verify(s, e, w, &result, expected);
    assert_eq!(result, minimum_cost_dominating_set(s, e, w).unwrap());
    for method in ["reduction", "fused", "heap", "quadratic"] {
        let mask = candidates::run(s, e, Some(w), method).unwrap();
        verify(s, e, w, &mask, expected);
        assert_eq!(mask, candidates::run(s, e, Some(w), method).unwrap());
        if ways == 1 {
            assert_eq!(mask, winner, "unique optimum for {method}");
        }
    }
    if ways == 1 {
        assert_eq!(result, winner);
    }
    (result, expected)
}

fn check_units(s: &[i64], e: &[i64]) -> Vec<bool> {
    let units = vec![1; s.len()];
    let (_, expected) = check(s, e, &units);
    let result = minimum_dominating_set(s, e).unwrap();
    verify(s, e, &units, &result, expected);
    assert_eq!(result, minimum_dominating_set(s, e).unwrap());
    for method in candidates::METHODS {
        let mask = candidates::run::<_, i128>(s, e, None, method).unwrap();
        verify(s, e, &units, &mask, expected);
        assert_eq!(
            mask,
            candidates::run::<_, i128>(s, e, None, method).unwrap()
        );
        let explicit = candidates::run(s, e, Some(&units), method).unwrap();
        verify(s, e, &units, &explicit, expected);
        let zeros = vec![0; s.len()];
        let zero_mask = candidates::run(s, e, Some(&zeros), method).unwrap();
        verify(s, e, &zeros, &zero_mask, (0, expected.1));
    }
    assert_eq!(check(s, e, &vec![0; s.len()]).1, (0, expected.1));
    result
}

#[test]
fn zero_rows_and_self_dominating_singletons_in_all_cost_modes() {
    assert_eq!(check(&[], &[], &[]), (vec![], (0, 0)));
    assert!(check_units(&[], &[]).is_empty());
    for end in [3, 7] {
        assert_eq!(check(&[3], &[end], &[0]), (vec![true], (0, 1)));
        assert_eq!(check(&[3], &[end], &[8]), (vec![true], (8, 1)));
        assert_eq!(check_units(&[3], &[end]), [true]);
    }
}

#[test]
fn every_empty_row_is_mandatory_even_duplicates_inside_or_at_endpoints() {
    assert_eq!(
        check(&[4, 4, 0, 9], &[4, 4, 0, 9], &[0, 7, 0, 2]),
        (vec![true; 4], (9, 4))
    );
    assert_eq!(check_units(&[4, 4, 0, 9], &[4, 4, 0, 9]), [true; 4]);
    assert_eq!(
        check(&[0, 0, 4, 4, 9], &[9, 0, 4, 4, 9], &[2, 0, 7, 0, 3]),
        (vec![true; 5], (12, 5))
    );
}

#[test]
fn touching_rows_are_isolated_but_duplicate_nonempty_rows_share_domination() {
    assert_eq!(check_units(&[0, 2], &[2, 4]), [true, true]);
    assert_eq!(check(&[0, 2], &[2, 4], &[0, 3]).1, (3, 2));
    assert_eq!(
        check(&[0; 3], &[5; 3], &[9, 1, 0]),
        (vec![false, false, true], (0, 1))
    );
}

#[test]
fn three_row_path_minimizes_cost_before_count_then_breaks_cost_ties_by_count() {
    let (s, e) = ([0, 3, 6], [4, 7, 10]);
    assert_eq!(check_units(&s, &e), [false, true, false]);
    assert_eq!(
        check(&s, &e, &[1, 10, 1]),
        (vec![true, false, true], (2, 2))
    );
    assert_eq!(
        check(&s, &e, &[1, 2, 1]),
        (vec![false, true, false], (2, 1))
    );
    assert_eq!(check(&s, &e, &[0; 3]), (vec![false, true, false], (0, 1)));
}

#[test]
fn weighted_optimum_may_require_overlapping_representatives() {
    assert_eq!(
        check(&[0, 1, 3, 5], &[2, 4, 6, 7], &[100, 1, 1, 100]),
        (vec![false, true, true, false], (2, 2))
    );
}

#[test]
fn star_chooses_hub_or_leaves_by_cost_and_adds_an_isolated_component() {
    let (s, e) = ([0, 1, 4, 7], [10, 2, 5, 8]);
    assert_eq!(check_units(&s, &e), [true, false, false, false]);
    assert_eq!(
        check(&s, &e, &[10, 1, 1, 1]),
        (vec![false, true, true, true], (3, 3))
    );
    assert_eq!(
        check(&[0, 1, 20, 4, 7], &[10, 2, 21, 5, 8], &[10, 1, 5, 1, 1]),
        (vec![false, true, true, true, true], (8, 4))
    );
}

#[test]
fn target_pruning_keeps_large_containing_rows_as_candidates() {
    assert_eq!(
        check(&[0, 1, 7], &[10, 2, 8], &[1, 100, 100]),
        (vec![true, false, false], (1, 1))
    );
}

#[test]
fn nested_cliques_endpoint_ties_and_equal_minimal_geometries() {
    for cheap in 0..3 {
        let mut w = [9; 3];
        w[cheap] = 1;
        let (mask, value) = check(&[0, 1, 2], &[10, 9, 8], &w);
        assert_eq!(value, (1, 1));
        assert!(mask[cheap]);
    }
    for (s, e) in [
        (vec![0, 0, 0], vec![10, 6, 3]),
        (vec![0, 3, 6], vec![10, 10, 10]),
        (vec![0, 1, 1, 3], vec![8, 4, 4, 6]),
        (vec![0, 1, 2, 3], vec![3, 4, 5, 6]),
    ] {
        check_units(&s, &e);
        check(&s, &e, &(0..s.len()).map(|i| i as i128).collect::<Vec<_>>());
    }
}

#[test]
fn already_dominated_rows_remain_eligible_greedy_representatives() {
    let (s, e) = ([0, 1, 3, 5, 8], [2, 5, 9, 6, 10]);
    assert_eq!(check_units(&s, &e).iter().filter(|&&m| m).count(), 2);
    // This exact mask tests the deliberate furthest-reaching private greedy
    // tie, without promising equal tied masks for different public algorithms.
    assert_eq!(
        candidates::run::<_, i128>(&s, &e, None, "greedy").unwrap(),
        [false, true, true, false, false]
    );
}

#[test]
fn interleaved_components_add_cost_and_restore_original_row_order() {
    let s = [103, 0, 106, 3, 100, 6, 50];
    let e = [107, 4, 110, 7, 104, 10, 50];
    let w = [10, 1, 1, 10, 1, 1, 0];
    assert_eq!(
        check(&s, &e, &w),
        (vec![false, true, true, false, true, true, true], (4, 5))
    );
    assert_eq!(
        check_units(&s, &e),
        [true, false, false, true, false, false, true]
    );
}

#[test]
fn zero_cost_vertices_remain_demands_but_redundant_selections_are_removed() {
    for (s, e) in [
        (vec![0, 3, 6, 20, 20], vec![4, 7, 10, 25, 25]),
        (vec![0, 1, 2, 3, 4, 30], vec![3, 4, 5, 6, 7, 30]),
        (vec![0, 0, 0, 9, 9], vec![5, 5, 5, 12, 12]),
    ] {
        check_units(&s, &e);
        let zeros = vec![0; s.len()];
        let (mask, expected) = check(&s, &e, &zeros);
        assert_eq!(expected.0, 0);
        assert!(mask.iter().any(|&m| m));
    }
}

#[test]
fn target_boundaries_batches_expirations_and_identical_blocks() {
    let s = [0, 4, 8, 12, 1, 0, 3, 3, 7, 11, -1];
    let e = [2, 6, 10, 14, 4, 7, 7, 8, 12, 15, 15];
    // [1,4) touches the second target; [3,8) touches the third. Rows 6/7
    // have the same block despite different geometry. Cheap short proposals
    // expire before longer, more expensive proposals buried in the heap.
    for w in [
        [7, 8, 9, 10, 0, 2, 1, 2, 1, 0, 100],
        [0, 0, 0, 0, 4, 5, 6, 7, 8, 9, 1],
    ] {
        check(&s, &e, &w);
    }
    check_units(&s, &e);
    check_reduction(&s[..8], &e[..8]);
}

#[test]
fn generic_endpoint_order_and_wide_integer_costs_are_exact() {
    fn generic<T: Ord + Copy + std::fmt::Debug>(s: &[T], e: &[T]) {
        let w = [u64::MAX, u64::MAX];
        assert_eq!(minimum_dominating_set(s, e).unwrap(), [true, true]);
        assert_eq!(minimum_cost_dominating_set(s, e, &w).unwrap(), [true, true]);
        for method in candidates::METHODS {
            assert_eq!(
                candidates::run(s, e, Some(&w), method).unwrap(),
                [true, true]
            );
        }
    }
    generic(&[i64::MIN, 0], &[0, i64::MAX]);
    generic(&[0u64, u64::MAX - 1], &[1, u64::MAX]);
    generic(&[i8::MIN, 0], &[0, i8::MAX]);
    generic(&[0u8, 254], &[1, 255]);
    generic(&['a', 'm'], &['m', 'z']);
    let big = (1i128 << 53) + 1;
    assert_eq!(
        check(&[0, 0], &[5, 5], &[big + 1, big]),
        (vec![false, true], (big, 1))
    );
    assert_eq!(
        check(&[0, 5], &[5, 10], &[u64::MAX.into(); 2]).1,
        (2 * i128::from(u64::MAX), 2)
    );
}

#[test]
fn checked_i128_arithmetic_rejects_only_unrepresentable_optima() {
    for method in ["production", "reduction", "fused", "heap", "quadratic"] {
        let run = |s: &[i64], e: &[i64], w: &[i128]| {
            if method == "production" {
                minimum_cost_dominating_set(s, e, w)
            } else {
                candidates::run(s, e, Some(w), method)
            }
        };
        let duplicate = run(&[0, 0], &[5, 5], &[i128::MAX; 2]).unwrap();
        assert_eq!(
            objective(&[i128::MAX; 2], &duplicate),
            (i128::MAX, 1),
            "{method}"
        );
        assert_eq!(
            run(&[0, 2], &[1, 3], &[i128::MAX, 1]),
            Err(IntervalError::CostOverflow),
            "{method}"
        );
        assert_eq!(
            run(&[0, 0], &[0, 0], &[i128::MAX, 1]),
            Err(IntervalError::CostOverflow),
            "{method}"
        );
        // Uniform costs use the cardinality fast path but still accumulate
        // the selected cost, including every isolated empty row.
        assert_eq!(
            run(&[0, 2], &[1, 3], &[i128::MAX; 2]),
            Err(IntervalError::CostOverflow),
            "{method}"
        );
        assert_eq!(
            run(&[0, 0], &[0, 0], &[i128::MAX; 2]),
            Err(IntervalError::CostOverflow),
            "{method}"
        );
        assert_eq!(
            run(&[0, 0], &[0, 1], &[i128::MAX, 0]).unwrap(),
            [true, true],
            "{method}"
        );
        assert_eq!(
            run(&[0, 0], &[0, 1], &[i128::MAX, 1]),
            Err(IntervalError::CostOverflow),
            "{method}"
        );
        assert_eq!(
            run(&[0, 3, 6], &[4, 7, 10], &[i128::MAX, 7, i128::MAX]).unwrap(),
            [false, true, false],
            "{method}"
        );
        assert_eq!(
            run(&[0, 3], &[2, 5], &[i128::MAX - 1, 1]).unwrap(),
            [true, true],
            "{method}"
        );
        assert_eq!(
            run(&[0], &[1], &[i128::MIN]),
            Err(IntervalError::NegativeCost { index: 0 }),
            "{method}"
        );
    }
}

#[test]
fn validation_precedes_pruning_empty_returns_and_uniform_fast_paths() {
    assert_eq!(
        minimum_dominating_set(&[0, 4], &[0]),
        Err(IntervalError::LengthMismatch([("starts", 2), ("ends", 1)]))
    );
    assert_eq!(
        minimum_dominating_set(&[0, 4], &[0, 3]),
        Err(IntervalError::InvalidInterval { index: 1 })
    );
    for method in [
        "production",
        "reduction",
        "fused",
        "heap",
        "quadratic",
        "greedy",
    ] {
        let run = |s: &[i64], e: &[i64], w: &[i128]| {
            if method == "production" {
                minimum_cost_dominating_set(s, e, w)
            } else {
                candidates::run(s, e, Some(w), method)
            }
        };
        assert_eq!(
            run(&[0, 4], &[0], &[1, 1]),
            Err(IntervalError::LengthMismatch([("starts", 2), ("ends", 1)])),
            "{method}"
        );
        assert_eq!(
            run(&[0], &[0], &[]),
            Err(IntervalError::LengthMismatch([
                ("intervals", 1),
                ("costs", 0)
            ])),
            "{method}"
        );
        assert_eq!(
            run(&[], &[], &[1]),
            Err(IntervalError::LengthMismatch([
                ("intervals", 0),
                ("costs", 1)
            ])),
            "{method}"
        );
        for (s, e) in [([0, 0, 4], [10, 0, 3]), ([0, 1, 4], [10, 2, 3])] {
            for w in [[0; 3], [1; 3]] {
                assert_eq!(
                    run(&s, &e, &w),
                    Err(IntervalError::InvalidInterval { index: 2 }),
                    "{method}"
                );
            }
        }
        for (s, e) in [
            ([0, 0, 0], [10, 10, 10]),
            ([0, 1, 2], [10, 9, 8]),
            ([4; 3], [4; 3]),
        ] {
            assert_eq!(
                run(&s, &e, &[0, 0, -1]),
                Err(IntervalError::NegativeCost { index: 2 }),
                "{method}"
            );
        }
    }
}

#[test]
fn adding_a_vertex_can_raise_or_lower_the_optimum_because_it_adds_a_demand() {
    assert_eq!(check(&[0], &[2], &[5]).1, (5, 1));
    assert_eq!(check(&[0, 3], &[2, 4], &[5, 2]).1, (7, 2));
    assert_eq!(check(&[0, 3, 1], &[2, 4, 5], &[5, 2, 1]).1, (1, 1));
}

// Pairwise strict containment is independent of optimized target extraction.
// Equal geometries are deduplicated ONLY after testing strict containment.
fn naive_targets(s: &[i64], e: &[i64]) -> Vec<(i64, i64)> {
    let mut targets: Vec<_> = (0..s.len())
        .filter(|&i| {
            s[i] < e[i]
                && !(0..s.len()).any(|j| {
                    s[j] < e[j] && s[i] <= s[j] && e[j] <= e[i] && (s[i] != s[j] || e[i] != e[j])
                })
        })
        .map(|i| (s[i], e[i]))
        .collect();
    targets.sort_unstable();
    targets.dedup();
    targets
}

fn check_reduction(s: &[i64], e: &[i64]) {
    let reduction = candidates::reduce(s, e).unwrap();
    assert_eq!(reduction.targets, naive_targets(s, e));
    assert!(
        reduction
            .targets
            .windows(2)
            .all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1)
    );
    assert_eq!(
        reduction.empties,
        (0..s.len()).filter(|&i| s[i] == e[i]).collect::<Vec<_>>()
    );
    let mut rows: Vec<_> = reduction.blocks.iter().map(|b| b.row).collect();
    rows.sort_unstable();
    assert_eq!(
        rows,
        (0..s.len()).filter(|&i| s[i] < e[i]).collect::<Vec<_>>()
    );
    for block in &reduction.blocks {
        assert!(block.start < block.end);
        for (j, &(ts, te)) in reduction.targets.iter().enumerate() {
            assert_eq!(
                block.start <= j && j < block.end,
                s[block.row] < te && ts < e[block.row],
                "row {} target {j}",
                block.row
            );
        }
    }
    for bits in 0usize..1 << s.len() {
        let mask: Vec<_> = (0..s.len()).map(|i| bits & (1 << i) != 0).collect();
        let reduced = reduction.empties.iter().all(|&i| mask[i])
            && (0..reduction.targets.len()).all(|j| {
                reduction
                    .blocks
                    .iter()
                    .any(|b| mask[b.row] && b.start <= j && j < b.end)
            });
        assert_eq!(dominates(s, e, &mask), reduced, "subset {bits:b}");
    }
}

fn interval() -> impl Strategy<Value = (i64, i64)> {
    prop_oneof![
        5 => (-6i64..=6, -6i64..=6).prop_map(|(a,b)| (a.min(b), a.max(b))),
        2 => (-5i64..=5, 1i64..=2).prop_map(|(block,len)| (4 * block, 4 * block + len)),
        2 => (0i64..=6).prop_map(|radius| (-radius, radius + 1)),
        1 => (-6i64..=6).prop_map(|x| (x,x)),
    ]
}

fn rows(max: usize) -> impl Strategy<Value = Vec<(i64, i64, i128, u32)>> {
    let cost = || prop_oneof![2 => Just(0i128), 3 => 1i128..=20];
    prop_oneof![
        4 => prop::collection::vec((interval(), cost(), any::<u32>()), 0..=max)
            .prop_map(|rows| rows.into_iter().map(|((s,e),w,key)| (s,e,w,key)).collect()),
        1 => (interval(), prop::collection::vec((cost(), any::<u32>()), 0..=max))
            .prop_map(|((s,e), rows)| rows.into_iter().map(|(w,key)| (s,e,w,key)).collect()),
    ]
}

fn columns(rows: &[(i64, i64, i128, u32)]) -> (Vec<i64>, Vec<i64>, Vec<i128>) {
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

fn solve(s: &[i64], e: &[i64], w: &[i128]) -> Objective {
    let mask = minimum_cost_dominating_set(s, e, w).unwrap();
    assert_eq!(mask.len(), s.len());
    assert!(dominates(s, e, &mask));
    objective(w, &mask)
}

// Default proptest configuration honors PROPTEST_CASES and persists minimized
// regressions beside this source. Higher-case local run:
// PROPTEST_CASES=2048 cargo test -p intervals-core --test minimum_cost_dominating_set
proptest! {
    #[test]
    fn every_candidate_matches_original_graph_subset_oracle(input in rows(10)) {
        let (s,e,w) = columns(&input);
        check(&s,&e,&w);
        check_units(&s,&e);
    }

    #[test]
    fn reduction_preserves_every_subset_and_matches_naive_targets_and_adjacency(input in rows(7)) {
        let (s,e,_) = columns(&input);
        check_reduction(&s,&e);
    }

    #[test]
    fn permutations_translations_and_order_relabeling_preserve_original_graph_optima(
        input in rows(10), delta in -1000i64..=1000,
    ) {
        let (s,e,w) = columns(&input);
        let (optimum,ways,_) = brute_force(&s,&e,&w);
        let mask = minimum_cost_dominating_set(&s,&e,&w).unwrap();
        let mut order: Vec<_> = (0..s.len()).collect();
        order.sort_by_key(|&i| input[i].3);
        let ps: Vec<_> = order.iter().map(|&i| s[i]).collect();
        let pe: Vec<_> = order.iter().map(|&i| e[i]).collect();
        let pw: Vec<_> = order.iter().map(|&i| w[i]).collect();
        let permuted = minimum_cost_dominating_set(&ps,&pe,&pw).unwrap();
        prop_assert_eq!(objective(&pw,&permuted), optimum);
        prop_assert!(dominates(&ps,&pe,&permuted));
        if ways == 1 {
            for (j,&i) in order.iter().enumerate() { prop_assert_eq!(permuted[j],mask[i]); }
        }
        let shifted_s: Vec<_> = s.iter().map(|x| x + delta).collect();
        let shifted_e: Vec<_> = e.iter().map(|x| x + delta).collect();
        prop_assert_eq!(minimum_cost_dominating_set(&shifted_s,&shifted_e,&w).unwrap(), &mask[..]);
        let mut endpoints: Vec<_> = s.iter().chain(&e).copied().collect();
        endpoints.sort_unstable(); endpoints.dedup();
        let relabel = |values: &[i64]| values.iter().map(|v| {
            let rank = endpoints.binary_search(v).unwrap() as i64;
            rank * rank * rank + 100 * rank - 500
        }).collect::<Vec<_>>();
        prop_assert_eq!(minimum_cost_dominating_set(&relabel(&s),&relabel(&e),&w).unwrap(), mask);
    }

    #[test]
    fn positive_scaling_and_individual_cost_monotonicity(
        input in rows(80), scale in 1i128..=20, index in any::<usize>(), change in 0i128..=20,
    ) {
        let (s,e,w) = columns(&input);
        let optimum = solve(&s,&e,&w);
        let scaled: Vec<_> = w.iter().map(|v| v * scale).collect();
        prop_assert_eq!(solve(&s,&e,&scaled), (optimum.0 * scale,optimum.1));
        if !w.is_empty() {
            let i = index % w.len();
            let mut increased = w.clone(); increased[i] += change;
            prop_assert!(solve(&s,&e,&increased).0 >= optimum.0);
            let mut decreased = w.clone(); decreased[i] = (w[i] - change).max(0);
            prop_assert!(solve(&s,&e,&decreased).0 <= optimum.0);
        }
    }

    #[test]
    fn mandatory_empty_addition_and_nonempty_clone_have_different_effects(
        input in rows(60), q in 0i128..=20, x in -6i64..=6, index in any::<usize>(),
    ) {
        let (mut s,mut e,mut w) = columns(&input);
        let optimum = solve(&s,&e,&w);
        s.push(x); e.push(x); w.push(q);
        prop_assert_eq!(solve(&s,&e,&w), (optimum.0 + q,optimum.1 + 1));
        // A second empty at exactly the same coordinate also adds its cost/count.
        s.push(x); e.push(x); w.push(0);
        prop_assert_eq!(solve(&s,&e,&w), (optimum.0 + q,optimum.1 + 2));
        let nonempty: Vec<_> = (0..input.len()).filter(|&i| s[i] < e[i]).collect();
        if !nonempty.is_empty() {
            let i = nonempty[index % nonempty.len()];
            s.truncate(input.len()); e.truncate(input.len()); w.truncate(input.len());
            s.push(s[i]); e.push(e[i]); w.push(w[i]);
            prop_assert_eq!(solve(&s,&e,&w), optimum);
            *w.last_mut().unwrap() += q;
            prop_assert_eq!(solve(&s,&e,&w), optimum);
        }
    }

    #[test]
    fn separated_union_adds_component_objectives(left in rows(35),right in rows(35)) {
        let (mut s,mut e,mut w) = columns(&left);
        let (rs,re,rw) = columns(&right);
        let a = solve(&s,&e,&w); let b = solve(&rs,&re,&rw);
        s.extend(rs.iter().map(|x| x + 100)); e.extend(re.iter().map(|x| x + 100)); w.extend(rw);
        prop_assert_eq!(solve(&s,&e,&w), (a.0 + b.0,a.1 + b.1));
    }

    #[test]
    fn enlarging_a_nonempty_vertex_cannot_worsen_the_objective(input in rows(80),index in any::<usize>()) {
        let (mut s,mut e,w) = columns(&input);
        let optimum = solve(&s,&e,&w);
        let nonempty: Vec<_> = (0..s.len()).filter(|&i| s[i] < e[i]).collect();
        if !nonempty.is_empty() {
            let i = nonempty[index % nonempty.len()];
            s[i] -= 30; e[i] += 30;
            prop_assert!(solve(&s,&e,&w) <= optimum);
        }
    }

    #[test]
    fn arbitrary_width_endpoints_use_ordered_pairs_without_coordinate_arithmetic(
        input in prop::collection::vec((any::<u64>(),any::<u64>(),0i128..=20),0..=8),
    ) {
        let s: Vec<_> = input.iter().map(|r| r.0.min(r.1)).collect();
        let e: Vec<_> = input.iter().map(|r| r.0.max(r.1)).collect();
        let w: Vec<_> = input.iter().map(|r| r.2).collect();
        let mut endpoints: Vec<_> = s.iter().chain(&e).copied().collect();
        endpoints.sort_unstable(); endpoints.dedup();
        let ranks = |values: &[u64]| values.iter().map(|x| endpoints.binary_search(x).unwrap() as i64).collect::<Vec<_>>();
        let expected = brute_force(&ranks(&s),&ranks(&e),&w).0;
        let mask = minimum_cost_dominating_set(&s,&e,&w).unwrap();
        prop_assert_eq!(mask.len(),s.len());
        prop_assert!(dominates(&s,&e,&mask));
        prop_assert_eq!(objective(&w,&mask),expected);
    }
}
