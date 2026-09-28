use intervals_core::{IntervalError, max_k_coverage, minimum_cover};
use proptest::prelude::*;

#[path = "support/coverage.rs"]
mod oracle;
use oracle::{brute, objective};

fn check(starts: &[i64], ends: &[i64], k: usize) -> (i128, usize) {
    let expected = brute(starts, ends, k);
    assert_eq!(oracle::quadratic(starts, ends, k), expected);
    let mask = max_k_coverage(starts, ends, k).unwrap();
    assert_eq!(
        objective(starts, ends, &mask),
        expected,
        "starts={starts:?}, ends={ends:?}, k={k}"
    );
    assert!(mask.iter().filter(|&&b| b).count() <= k);
    expected
}

fn solve(s: &[i64], e: &[i64], k: usize) -> (i128, usize) {
    objective(s, e, &max_k_coverage(s, e, k).unwrap())
}

fn intervals() -> impl Strategy<Value = (Vec<i64>, Vec<i64>)> {
    prop::collection::vec((-8i64..=12, -8i64..=12), 0..=11)
        .prop_map(|rows| rows.into_iter().map(|(a, b)| (a.min(b), a.max(b))).unzip())
}

macro_rules! examples {
    ($($name:ident: $s:expr, $e:expr, $k:expr => $expected:expr;)*) => {$(
        #[test] fn $name() { assert_eq!(check(&$s, &$e, $k), $expected); }
    )*};
}
examples! {
    empty_input: [], [], 99 => (0,0);
    zero_budget: [0,2], [5,6], 0 => (0,0);
    single_interval: [2], [7], 3 => (5,1);
    only_empty: [5], [5], 3 => (0,0);
    longest: [0,10,2], [5,20,9], 1 => (10,1);
    disjoint: [0,10,20], [3,15,27], 2 => (12,2);
    identical: [0;8], [10;8], 8 => (10,1);
    deeply_nested: [0,2,4,6], [20,18,16,14], 9 => (20,1);
    touching: [0,5], [5,10], 2 => (10,2);
    overlap_arithmetic: [0,5], [10,15], 2 => (15,2);
    top_lengths_counterexample: [0,1,10], [10,11,18], 2 => (18,2);
    longest_first_counterexample: [0,-5,6], [10,4,15], 2 => (18,2);
    duplicates_and_complement: [0,0,8], [10,10,20], 3 => (20,2);
    equal_starts: [0,0,0,10], [4,8,10,15], 2 => (15,2);
    equal_ends: [0,2,5,10], [10,10,10,15], 3 => (15,2);
    mixed_empty: [0,3,5,9], [5,3,10,9], 4 => (10,2);
    signed_extremes: [i64::MIN,0,1], [0,i64::MAX,i64::MAX], 2 => (u64::MAX as i128,2);
}

#[test]
fn unique_masks_and_original_order() {
    assert_eq!(
        max_k_coverage(&[0, 10, 2], &[5, 20, 9], 1).unwrap(),
        [false, true, false]
    );
    assert_eq!(
        max_k_coverage(&[0, 10, 20], &[3, 15, 27], 2).unwrap(),
        [false, true, true]
    );
    assert_eq!(
        max_k_coverage(&[0, 2, 4, 6], &[20, 18, 16, 14], 5).unwrap(),
        [true, false, false, false]
    );
    assert_eq!(
        max_k_coverage(&[6, 0, -5], &[15, 10, 4], 2).unwrap(),
        [true, false, true]
    );
}

#[test]
fn budget_sensitivity_and_saturation() {
    let (s, e) = ([0, -5, 6, 30, 0], [10, 4, 15, 33, 10]);
    for (k, expected) in [
        (0, (0, 0)),
        (1, (10, 1)),
        (2, (18, 2)),
        (3, (21, 3)),
        (4, (23, 4)),
        (5, (23, 4)),
    ] {
        assert_eq!(check(&s, &e, k), expected);
    }
    for k in 2..=6 {
        assert_eq!(check(&[0, 5, 2, 7], &[5, 10, 4, 9], k), (10, 2));
    }
}

#[test]
fn unsigned_extremes_and_validation() {
    let s = [0u64, u64::MAX - 1];
    let e = [u64::MAX - 1, u64::MAX];
    let mask = max_k_coverage(&s, &e, 2).unwrap();
    assert_eq!(objective(&s, &e, &mask), (i128::from(u64::MAX), 2));
    for k in [0, 1, 10, usize::MAX] {
        assert_eq!(
            max_k_coverage(&[0, 4, 5], &[0, 3, 1], k),
            Err(IntervalError::InvalidInterval { index: 1 })
        );
        assert_eq!(
            max_k_coverage(&[0], &[], k),
            Err(IntervalError::LengthMismatch([("starts", 1), ("ends", 0)]))
        );
    }
}

#[test]
fn continuous_union_agrees_with_minimum_cover() {
    let s = [0, 2, 5, 7, 1];
    let e = [4, 6, 9, 10, 8];
    let cover = minimum_cover(&s, &e, 0, 10).unwrap();
    let count = cover.iter().filter(|&&b| b).count();
    assert_eq!(solve(&s, &e, count), (10, count));
    assert_eq!(solve(&s, &e, 100), (10, count));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(384))]

    #[test]
    fn exhaustive_objective((s,e) in intervals(), k in 0usize..=12) {
        check(&s,&e,k);
        let mask = max_k_coverage(&s,&e,k).unwrap();
        prop_assert_eq!(mask.len(),s.len());
        prop_assert_eq!(&mask,&max_k_coverage(&s,&e,k).unwrap());
        prop_assert!(mask.iter().filter(|&&b|b).count() <= k);
        prop_assert!(s.iter().zip(&e).zip(&mask).all(|((&s,&e),&b)| !b || s<e));
        let value = objective(&s,&e,&mask).0;
        prop_assert!(value <= objective(&s,&e,&vec![true;s.len()]).0);
        let sum: i128 = s.iter().zip(&e).zip(&mask).filter(|(_,b)| **b).map(|((&s,&e),_)| i128::from(e)-i128::from(s)).sum();
        prop_assert!(value <= sum);
    }

    #[test]
    fn budgets_monotone_saturating_and_base_cases((s,e) in intervals(), k in 0usize..=12) {
        prop_assert!(solve(&s,&e,k).0 <= solve(&s,&e,k+1).0);
        prop_assert_eq!(solve(&s,&e,s.len()+10).0, objective(&s,&e,&vec![true;s.len()]).0);
        prop_assert_eq!(solve(&s,&e,0),(0,0));
        let max_length = s.iter().zip(&e).map(|(&s,&e)|i128::from(e-s)).max().unwrap_or(0);
        prop_assert_eq!(solve(&s,&e,1),(max_length,usize::from(max_length>0)));
    }

    #[test]
    fn geometry_and_permutation_invariance((s,e) in intervals(), k in 0usize..=12,
        offset in -100i64..=100, scale in 1i64..=20, keys in prop::collection::vec(any::<u32>(),11)) {
        let expected = solve(&s,&e,k);
        let mut order: Vec<_> = (0..s.len()).collect(); order.sort_by_key(|&i|keys[i]);
        let ps: Vec<_> = order.iter().map(|&i|s[i]).collect();
        let pe: Vec<_> = order.iter().map(|&i|e[i]).collect();
        prop_assert_eq!(solve(&ps,&pe,k),expected);
        let ts: Vec<_> = s.iter().map(|s|s+offset).collect();
        let te: Vec<_> = e.iter().map(|e|e+offset).collect();
        prop_assert_eq!(solve(&ts,&te,k),expected);
        prop_assert_eq!(max_k_coverage(&ts,&te,k).unwrap(),max_k_coverage(&s,&e,k).unwrap());
        let ss: Vec<_> = s.iter().map(|s|s*scale).collect();
        let se: Vec<_> = e.iter().map(|e|e*scale).collect();
        prop_assert_eq!(solve(&ss,&se,k),(expected.0*i128::from(scale),expected.1));
    }

    #[test]
    fn insertion_properties((s,e) in intervals(), k in 0usize..=12, a in -8i64..=12, b in -8i64..=12) {
        let expected = solve(&s,&e,k);
        let (mut ns,mut ne) = (s.clone(),e.clone());
        ns.push(a.min(b)); ne.push(a.max(b));
        prop_assert!(solve(&ns,&ne,k).0 >= expected.0);
        *ns.last_mut().unwrap() = a; *ne.last_mut().unwrap() = a;
        prop_assert_eq!(solve(&ns,&ne,k),expected);
        prop_assert!(!max_k_coverage(&ns,&ne,k).unwrap()[s.len()]);
        if !s.is_empty() {
            *ns.last_mut().unwrap() = s[0]; *ne.last_mut().unwrap() = e[0];
            prop_assert_eq!(solve(&ns,&ne,k),expected);
        }
    }

    #[test]
    fn disjoint_top_k(lengths in prop::collection::vec(1i64..=25,0..=20), k in 0usize..=25) {
        let s: Vec<_> = (0..lengths.len()).map(|i|i as i64*30).collect();
        let e: Vec<_> = s.iter().zip(&lengths).map(|(s,l)|s+l).collect();
        let mut sorted = lengths.clone(); sorted.sort_unstable_by(|a,b|b.cmp(a));
        prop_assert_eq!(solve(&s,&e,k),(sorted.iter().take(k).map(|&v|i128::from(v)).sum(),k.min(s.len())));
    }

    #[test]
    fn nested_and_duplicate_families(n in 1usize..=20, k in 1usize..=25) {
        let s: Vec<_> = (0..n).map(|i|i as i64).collect();
        let e: Vec<_> = s.iter().map(|&v|100-v).collect();
        prop_assert_eq!(solve(&s,&e,k),(100,1));
        prop_assert_eq!(solve(&vec![3;n],&vec![10;n],k),(7,1));
    }

    #[test]
    fn separated_component_convolution((a,b) in intervals(), (c,d) in intervals(), k in 0usize..=8) {
        let mut s = a.clone(); s.extend(c.iter().map(|x|x+100));
        let mut e = b.clone(); e.extend(d.iter().map(|x|x+100));
        let mut best = (0,0);
        for j in 0..=k {
            let x = brute(&a,&b,j); let y = brute(&c,&d,k-j);
            let v = (x.0+y.0,x.1+y.1);
            if v.0 > best.0 || (v.0 == best.0 && v.1 < best.1) {best=v;}
        }
        prop_assert_eq!(solve(&s,&e,k),best);
    }

    #[test]
    fn larger_instances_against_independent_quadratic_dp(
        rows in prop::collection::vec((-1000i64..=1000,-1000i64..=1000),12..=64),
        k in 0usize..=20,
    ) {
        let (s,e): (Vec<_>,Vec<_>) = rows.into_iter().map(|(a,b)|(a.min(b),a.max(b))).unzip();
        prop_assert_eq!(solve(&s,&e,k),oracle::quadratic(&s,&e,k));
    }
}
