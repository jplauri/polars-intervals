//! Reproducible release experiments; see docs/capacity-profile-benchmarks.md.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/random.rs"]
mod random;
#[path = "support/profile_reference.rs"]
mod reference;
use random::{random, shuffle};
use reference::{Job, Segment};
use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

// "scalar" times the constant-capacity API on the same instance.
fn run(name: &str, jobs: &[Job], profile: &[Segment]) -> Vec<bool> {
    let s: Vec<_> = jobs.iter().map(|r| r.0).collect();
    let e: Vec<_> = jobs.iter().map(|r| r.1).collect();
    let w: Vec<_> = jobs.iter().map(|r| r.2).collect();
    if name == "scalar" {
        let capacity = profile.first().map_or(0, |r| r.2);
        return intervals_core::max_weight_with_capacity(&s, &e, &w, capacity).unwrap();
    }
    let ps: Vec<_> = profile.iter().map(|r| r.0).collect();
    let pe: Vec<_> = profile.iter().map(|r| r.1).collect();
    let c: Vec<_> = profile.iter().map(|r| r.2 as u64).collect();
    intervals_core::max_weight_with_capacity_profile(&s, &e, &w, &ps, &pe, &c).unwrap()
}

fn validate() {
    let mut state = 781;
    for _ in 0..256 {
        let jobs: Vec<_> = (0..random(&mut state) % 11)
            .map(|_| {
                let a = (random(&mut state) % 17) as i64 - 6;
                let b = (random(&mut state) % 17) as i64 - 6;
                (a.min(b), a.max(b), (random(&mut state) % 31) as i64 - 10)
            })
            .collect();
        let mut profile: Vec<_> = (-6..10)
            .filter_map(|t| {
                let c = random(&mut state) as usize % 5;
                (c > 0).then_some((t, t + 1, c))
            })
            .collect();
        shuffle(&mut profile, &mut state);
        let optimum = reference::brute_force(&jobs, &profile);
        let mask = run("production", &jobs, &profile);
        assert_eq!(reference::verify(&jobs, &profile, &mask), optimum);
    }
    eprintln!("256 exhaustive random instances verified");
}

fn dataset(n: usize, m: usize, k: usize, family: &str, pattern: &str) -> (Vec<Job>, Vec<Segment>) {
    let mut state = 42;
    let mut jobs: Vec<_> = (0..n as i64)
        .map(|i| {
            let (s, e) = match family {
                "disjoint" => (i * 3, i * 3 + 2),
                "touching" => (i, i + 1),
                "sparse" => (i, i + 3),
                "moderate" | "negative" | "valuable_long" | "bottleneck_short" => (i, i + 32),
                "dense" => (i, i + n as i64),
                "nested" => (i, 2 * n as i64 - i),
                "identical" => (0, 10_000),
                "empties" => (i, i + if i % 4 == 0 { 32 } else { 0 }),
                "components" => (i / 32 * 100 + i % 32, i / 32 * 100 + i % 32 + 16),
                "outside" => (i - n as i64, i - n as i64 + 32),
                "spanning_gaps" => (i, i + 500),
                _ => unreachable!(),
            };
            let w = (random(&mut state) % 100) as i64 + 1;
            (
                s,
                e,
                if family == "negative" && i % 4 != 0 {
                    -w
                } else {
                    w
                },
            )
        })
        .collect();
    let horizon = jobs.iter().map(|r| r.1).max().unwrap_or(1).max(m as i64);
    if family == "valuable_long" && !jobs.is_empty() {
        jobs[0] = (0, horizon, 5 * n as i64);
    }
    let profile = (0..m)
        .map(|j| {
            let c = match pattern {
                "constant" => k,
                "zero" => 0,
                "bottleneck" => {
                    if j == m / 2 {
                        1
                    } else {
                        k
                    }
                }
                "increasing" => k.saturating_mul(1usize << (j * 4 / m)),
                "decreasing" => k.saturating_mul(1usize << (3 - j * 4 / m)),
                "sawtooth" => {
                    if j % 2 == 0 {
                        1
                    } else {
                        k
                    }
                }
                "small_changes" => k + j % 2,
                "zero_gaps" => {
                    if j % 4 == 1 || j % 4 == 2 {
                        0
                    } else {
                        k
                    }
                }
                "high" => n + j % 2,
                _ => unreachable!(),
            };
            (
                j as i64 * horizon / m as i64,
                (j + 1) as i64 * horizon / m as i64,
                c,
            )
        })
        .collect();
    (jobs, profile)
}

#[derive(Clone)]
struct Case {
    n: usize,
    m: usize,
    k: usize,
    family: &'static str,
    pattern: &'static str,
    order: &'static str,
}
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut add = |n, m, k, family, pattern, order| {
        cases.push(Case {
            n,
            m,
            k,
            family,
            pattern,
            order,
        })
    };
    for m in [0, 1, 16] {
        add(0, m, 0, "disjoint", "zero", "sorted");
    }
    // Orthogonal slices cover the requested dimensions without their Cartesian product.
    for family in [
        "disjoint",
        "sparse",
        "moderate",
        "dense",
        "nested",
        "identical",
        "touching",
        "empties",
        "negative",
        "valuable_long",
        "bottleneck_short",
        "outside",
        "spanning_gaps",
        "components",
    ] {
        for pattern in [
            "constant",
            "bottleneck",
            "increasing",
            "decreasing",
            "sawtooth",
            "small_changes",
            "zero_gaps",
            "high",
        ] {
            add(1_000, 16, 4, family, pattern, "sorted");
        }
    }
    for k in [1, 2, 4, 8, 16, 64, 256, 1_000, 1_000_000_000] {
        add(1_000, 16, k, "dense", "bottleneck", "sorted");
        add(1_000, 1, k, "moderate", "constant", "sorted");
    }
    for m in [1, 4, 16, 100, 1_000, 10_000] {
        add(10_000, m, 4, "moderate", "small_changes", "sorted");
        add(10_000, m, 4, "disjoint", "high", "shuffled_profile");
    }
    for order in ["shuffled_jobs", "shuffled_profile", "both_shuffled"] {
        add(1_000, 100, 4, "moderate", "sawtooth", order);
        add(10_000, 1_000, 4, "components", "small_changes", order);
    }
    for n in [10_000, 100_000, 1_000_000] {
        for family in [
            "disjoint",
            "touching",
            "identical",
            "components",
            "negative",
        ] {
            add(n, 16, 4, family, "high", "sorted");
        }
        add(n, 16, 4, "components", "small_changes", "sorted");
        add(n, 16, 4, "disjoint", "constant", "sorted");
    }
    for n in [1_000, 10_000, 100_000, 1_000_000] {
        add(n, 16, 0, "outside", "zero", "sorted");
        add(n, 0, 0, "outside", "zero", "sorted");
    }
    for family in ["dense", "identical"] {
        for k in [256, 1_000] {
            for m in [16, 100] {
                add(1_000, m, k, family, "sawtooth", "sorted");
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    cases.retain(|c| seen.insert((c.n, c.m, c.k, c.family, c.pattern, c.order)));
    cases
}
fn main() {
    if cfg!(debug_assertions) {
        panic!("run release mode");
    }
    validate();
    let max_n = std::env::var("PROFILE_BENCH_MAX_N")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1_000_000);
    let samples = std::env::var("PROFILE_BENCH_SAMPLES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(3);
    let only = std::env::var("PROFILE_BENCH_FAMILY").ok();
    let min_n: usize = std::env::var("PROFILE_BENCH_MIN_N")
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    let min_m: usize = std::env::var("PROFILE_BENCH_MIN_M")
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    let mut state = 793;
    println!(
        "family,pattern,order,n,m,k,objective,algorithm,sample,ns,peak_allocated_bytes,allocations"
    );
    for case in cases().into_iter().filter(|c| {
        c.n <= max_n && c.n >= min_n && c.m >= min_m && only.as_ref().is_none_or(|f| f == c.family)
    }) {
        let Case {
            n,
            m,
            k,
            family,
            pattern,
            order,
        } = case;
        let (mut jobs, mut profile) = dataset(n, m, k, family, pattern);
        if order == "shuffled_jobs" || order == "both_shuffled" {
            shuffle(&mut jobs, &mut state);
        }
        if order == "shuffled_profile" || order == "both_shuffled" {
            shuffle(&mut profile, &mut state);
        }
        let mut methods = vec!["production"];
        if (pattern == "constant" && family != "outside") || pattern == "zero" {
            methods.push("scalar");
        }
        let expected = reference::verify(&jobs, &profile, &run("production", &jobs, &profile));
        if pattern == "high" && family != "outside" {
            assert_eq!(
                expected,
                jobs.iter().map(|r| i128::from(r.2.max(0))).sum::<i128>()
            );
        }
        let tiny = &jobs[..jobs.len().min(10)];
        // Restrict the profile to the tiny job horizon. Capacity outside it
        // cannot affect any subset; this keeps the deliberately naive oracle
        // tiny even when the benchmark has ten thousand profile records.
        let lo = tiny.iter().map(|r| r.0).min().unwrap_or(0);
        let hi = tiny.iter().map(|r| r.1).max().unwrap_or(0);
        let tiny_profile: Vec<_> = profile
            .iter()
            .filter_map(|&(s, e, c)| (s.max(lo) < e.min(hi)).then_some((s.max(lo), e.min(hi), c)))
            .collect();
        let oracle = reference::brute_force(tiny, &tiny_profile);
        for &method in &methods {
            assert_eq!(
                reference::verify(tiny, &profile, &run(method, tiny, &profile)),
                oracle
            );
        }
        let mut memory = std::collections::BTreeMap::new();
        for &method in &methods {
            let (mask, bytes, allocs) = allocations::measure(|| run(method, &jobs, &profile));
            assert_eq!(
                reference::verify(&jobs, &profile, &mask),
                expected,
                "{method}: {family}/{pattern}"
            );
            memory.insert(method, (bytes, allocs));
        }
        for sample in 0..samples {
            shuffle(&mut methods, &mut state);
            for &method in &methods {
                let start = Instant::now();
                let mask = black_box(run(method, black_box(&jobs), black_box(&profile)));
                let ns = start.elapsed().as_nanos();
                assert_eq!(reference::verify(&jobs, &profile, &mask), expected);
                let (bytes, allocs) = memory[method];
                println!(
                    "{family},{pattern},{order},{n},{m},{k},{expected},{method},{sample},{ns},{bytes},{allocs}"
                );
            }
        }
        eprintln!("verified/measured n={n} m={m} k={k} {family}/{pattern}/{order}");
    }
}
