#[path = "../benches/support/profile_candidates.rs"]
#[allow(dead_code)]
mod candidates;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn all_three_flow_formulations_match_exhaustive_oracle(
        raw in prop::collection::vec((-6i64..=10, -6i64..=10, -10i64..=20), 0..=10),
        capacities in prop::collection::vec(0usize..=4, 16),
    ) {
        let jobs: Vec<_> = raw.into_iter().map(|(a,b,w)| (a.min(b),a.max(b),w)).collect();
        let profile: Vec<_> = capacities.into_iter().enumerate().filter_map(|(j,c)| (c > 0).then_some((j as i64 - 6,j as i64 - 5,c))).rev().collect();
        let optimum = candidates::brute_force(&jobs,&profile);
        for algorithm in ["A","B","C","guarded","prefilter","components","parallel"] {
            let result = candidates::run(algorithm,&jobs,&profile);
            prop_assert_eq!(candidates::verify(&jobs,&profile,&result.mask),optimum,"{}",algorithm);
        }
        prop_assert_eq!(candidates::normalize(&profile,false,jobs.len()),candidates::normalize(&profile,true,jobs.len()));
    }
}
