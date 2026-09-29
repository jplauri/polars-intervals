//! cargo bench -p intervals-core --bench max_weight_with_capacity --locked
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/random.rs"]
mod random;
#[path = "support/capacity_reference.rs"]
mod reference;
use random::{random, shuffle};

use intervals_core::{max_weight_non_overlapping, max_weight_with_capacity};
use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

// Workload descriptors over positive nonempty rows: peak concurrency, and
// overlap components in which touching rows are separate.
fn structure(s: &[i64], e: &[i64], w: &[i64]) -> (usize, usize) {
    let mut rows: Vec<_> = (0..s.len())
        .filter(|&i| s[i] < e[i] && w[i] > 0)
        .map(|i| (s[i], e[i]))
        .collect();
    rows.sort_unstable();
    let mut events: Vec<_> = rows
        .iter()
        .flat_map(|&(s, e)| [(s, 1i64), (e, -1)])
        .collect();
    events.sort_unstable();
    let (mut live, mut peak) = (0, 0);
    for (_, delta) in events {
        live += delta;
        peak = peak.max(live);
    }
    let mut components = 0;
    let mut reach = None;
    for (s, e) in rows {
        if reach.is_none_or(|r| s >= r) {
            components += 1;
        }
        reach = Some(reach.map_or(e, |r: i64| r.max(e)));
    }
    (peak as usize, components)
}

fn data(n: usize, family: &str, distribution: &str) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let mut state = 42;
    let mut rows: Vec<_> = (0..n as i64)
        .map(|i| {
            let (s, e) = match family {
                "disjoint" => (3 * i, 3 * i + 2),
                "sparse" => (i, i + 3),
                "moderate" => (i, i + 32),
                "dense" => (i, i + n as i64),
                "components" => (i / 32 * 100 + i % 32, i / 32 * 100 + i % 32 + 16),
                "nested" => (i, 2 * n as i64 - i),
                "staircase" => (i, i + 2),
                "identical" => (0, 10),
                "equal_endpoints" => (i / 16 * 8, i / 16 * 8 + 8 + i % 4),
                "touching" => (i, i + 1),
                "empties" => (i / 4, i / 4 + if i % 3 == 0 { 8 } else { 0 }),
                _ => unreachable!(),
            };
            let r = random(&mut state);
            let w = match distribution {
                "positive" => 1 + (r % 100) as i64,
                "mixed" => (r % 201) as i64 - 100,
                "zeros" => {
                    if r.is_multiple_of(5) {
                        1 + (r % 100) as i64
                    } else {
                        0
                    }
                }
                "equal" => 10,
                "ties" => 1 + (r % 3) as i64,
                "skewed" => 1i64 << (r % 63),
                "valuable_long" => 10,
                _ => unreachable!(),
            };
            (s, e, w)
        })
        .collect();
    if distribution == "valuable_long" && n > 0 {
        let end = rows.iter().map(|r| r.1).max().unwrap();
        rows[0] = (0, end, 5 * n as i64);
    }
    (
        rows.iter().map(|r| r.0).collect(),
        rows.iter().map(|r| r.1).collect(),
        rows.iter().map(|r| r.2).collect(),
    )
}

fn main() {
    if cfg!(debug_assertions) {
        panic!("run in release mode");
    }
    let max_n = std::env::var("CAPACITY_BENCH_MAX_N")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1_000_000);
    let samples = std::env::var("CAPACITY_BENCH_SAMPLES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(3);
    let only = std::env::var("CAPACITY_BENCH_FAMILY").ok();
    println!(
        "family,order,weights,n,k,concurrency,components,objective,algorithm,sample,ns,peak_allocated_bytes,allocations"
    );
    let mut state = 73;
    for n in [0, 1_000, 10_000, 100_000, 1_000_000]
        .into_iter()
        .filter(|&n| n <= max_n)
    {
        for family in [
            "disjoint",
            "sparse",
            "moderate",
            "dense",
            "components",
            "nested",
            "staircase",
            "identical",
            "equal_endpoints",
            "touching",
            "empties",
        ] {
            if only.as_ref().is_some_and(|value| value != family) {
                continue;
            }
            // All distributions at 1K; positive/mixed scaling at 10K; structural
            // scaling at 100K/1M. Not an unbounded Cartesian product.
            let distributions: &[&str] = if n <= 1_000 {
                &[
                    "positive",
                    "mixed",
                    "zeros",
                    "equal",
                    "ties",
                    "skewed",
                    "valuable_long",
                ]
            } else if n == 10_000 {
                &["positive", "mixed"]
            } else {
                &["positive"]
            };
            for &distribution in distributions {
                if n == 0 && (family != "disjoint" || distribution != "positive") {
                    continue;
                }
                let (ss, ee, ww) = data(n, family, distribution);
                for order in ["sorted", "shuffled"] {
                    let mut permutation: Vec<_> = (0..n).collect();
                    if order == "shuffled" {
                        shuffle(&mut permutation, &mut state);
                    }
                    let s: Vec<_> = permutation.iter().map(|&i| ss[i]).collect();
                    let e: Vec<_> = permutation.iter().map(|&i| ee[i]).collect();
                    let w: Vec<_> = permutation.iter().map(|&i| ww[i]).collect();
                    let (peak, count) = structure(&s, &e, &w);
                    let mut capacities = vec![0usize, 1, 2, 4, 8, 16, 64];
                    capacities.extend([peak.saturating_sub(1), peak, peak.saturating_add(1)]);
                    capacities.sort_unstable();
                    capacities.dedup();
                    for k in capacities {
                        // Bound the reference flow's n*k work, including near-clique
                        // capacities. The full suite reports these omissions.
                        if n.saturating_mul(k.min(peak)) > 2_000_000 {
                            eprintln!(
                                "omitted reference-flow work budget: {family}/{distribution}/{order} n={n} k={k} peak={peak}"
                            );
                            continue;
                        }
                        let expected =
                            reference::verify(&s, &e, &w, k, &reference::solve(&s, &e, &w, k));
                        // Structural independent oracles on full large instances.
                        if k == 0 {
                            assert_eq!(
                                expected,
                                (0..n)
                                    .filter(|&i| s[i] == e[i])
                                    .map(|i| i128::from(w[i].max(0)))
                                    .sum::<i128>()
                            );
                        } else if k >= peak {
                            assert_eq!(
                                expected,
                                w.iter().map(|&w| i128::from(w.max(0))).sum::<i128>()
                            );
                        } else if family == "identical" || family == "dense" || family == "nested" {
                            let mut positive: Vec<_> =
                                w.iter().copied().filter(|&w| w > 0).collect();
                            positive.sort_unstable_by(|a, b| b.cmp(a));
                            assert_eq!(
                                expected,
                                positive.into_iter().take(k).map(i128::from).sum::<i128>()
                            );
                        }
                        if k == 1 {
                            assert_eq!(
                                expected,
                                reference::verify(
                                    &s,
                                    &e,
                                    &w,
                                    k,
                                    &max_weight_non_overlapping(&s, &e, &w).unwrap()
                                )
                            );
                        }
                        // Brute force an independently chosen small restriction
                        // of EVERY case.
                        let tiny = n.min(10);
                        let (ts, te, tw) = (&s[..tiny], &e[..tiny], &w[..tiny]);
                        assert_eq!(
                            reference::verify(
                                ts,
                                te,
                                tw,
                                k,
                                &max_weight_with_capacity(ts, te, tw, k).unwrap()
                            ),
                            reference::brute_force(ts, te, tw, k)
                        );
                        let (mask, bytes, allocs) =
                            allocations::measure(|| max_weight_with_capacity(&s, &e, &w, k));
                        assert_eq!(reference::verify(&s, &e, &w, k, &mask.unwrap()), expected);
                        // The verified untimed call above warms the kernel.
                        for sample in 0..samples {
                            let begin = Instant::now();
                            let mask = black_box(max_weight_with_capacity(
                                black_box(&s),
                                black_box(&e),
                                black_box(&w),
                                k,
                            ));
                            let ns = begin.elapsed().as_nanos();
                            assert_eq!(reference::verify(&s, &e, &w, k, &mask.unwrap()), expected);
                            println!(
                                "{family},{order},{distribution},{n},{k},{peak},{count},{expected},production,{sample},{ns},{bytes},{allocs}"
                            );
                        }
                    }
                }
            }
            eprintln!("verified/measured n={n} family={family}");
        }
    }
}
