//! Reproducible release experiments; see docs/capacity-profile-benchmarks.md.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/profile_candidates.rs"]
mod candidates;
#[path = "support/random.rs"]
mod random;
use candidates::{Job, Measurement, Segment};
use random::{random, shuffle};
use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn run(name: &str, jobs: &[Job], profile: &[Segment]) -> Measurement {
    if name.starts_with("csr") {
        return run_csr(name, jobs, profile);
    }
    if name == "production" || name == "scalar" {
        let s: Vec<_> = jobs.iter().map(|r| r.0).collect();
        let e: Vec<_> = jobs.iter().map(|r| r.1).collect();
        let w: Vec<_> = jobs.iter().map(|r| r.2).collect();
        let mask = if name == "scalar" {
            intervals_core::max_weight_with_capacity(&s, &e, &w, profile.first().map_or(0, |r| r.2))
                .unwrap()
        } else {
            let ps: Vec<_> = profile.iter().map(|r| r.0).collect();
            let pe: Vec<_> = profile.iter().map(|r| r.1).collect();
            let c: Vec<_> = profile.iter().map(|r| r.2 as u64).collect();
            intervals_core::max_weight_with_capacity_profile(&s, &e, &w, &ps, &pe, &c).unwrap()
        };
        return Measurement {
            mask,
            ..Measurement::default()
        };
    }
    candidates::run(name, jobs, profile)
}
fn validate_candidates() {
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
        let optimum = candidates::brute_force(&jobs, &profile);
        for name in [
            "production",
            "csr",
            "csr_C",
            "csr_adaptive",
            "csr_budget",
            "csr_tight",
            "A",
            "B",
            "C",
            "guarded",
            "prefilter",
            "components",
            "parallel",
        ] {
            let result = run(name, &jobs, &profile);
            assert_eq!(
                candidates::verify(&jobs, &profile, &result.mask),
                optimum,
                "{name}"
            );
        }
        assert_eq!(
            candidates::normalize(&profile, false, jobs.len()),
            candidates::normalize(&profile, true, jobs.len())
        );
    }
    eprintln!("256 exhaustive random instances verified for all exact candidates");
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
    validate_candidates();
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
    let method_filter = std::env::var("PROFILE_BENCH_METHODS").ok();
    let mut state = 793;
    println!(
        "family,pattern,order,n,m,k,objective,algorithm,sample,ns,normalization_ns,preprocessing_ns,timeline_ns,solver_ns,reconstruction_ns,vertices,edges,augmentations,components,peak_allocated_bytes,allocations"
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
        let mut methods = vec!["production", "packed", "indirect"];
        if (pattern == "constant" && family != "outside") || pattern == "zero" {
            methods.push("scalar");
        }
        if n <= 1_000 {
            methods.extend([
                "csr",
                "csr_C",
                "csr_adaptive",
                "csr_budget",
                "csr_tight",
                "csr_guarded",
                "csr_prefilter",
                "csr_components",
                "csr_parallel",
                "A",
                "B",
                "C",
                "guarded",
                "prefilter",
                "components",
                "parallel",
            ]);
        } else if n <= 10_000 {
            methods.extend([
                "csr",
                "csr_C",
                "csr_adaptive",
                "csr_budget",
                "csr_tight",
                "csr_guarded",
                "csr_prefilter",
                "csr_components",
                "csr_parallel",
                "A",
                "B",
                "guarded",
                "prefilter",
                "components",
                "parallel",
            ]);
            if family == "moderate" && m >= 1_000 {
                methods.push("C");
            }
        } else {
            methods.push("guarded");
            if family == "components" && pattern != "high" {
                methods.extend(["csr_components", "csr_parallel", "components", "parallel"]);
            }
        }
        if n > 1_000 {
            // C's saturated correction can require one path per disjoint job.
            // Its larger-scale useful regime is the many-change short-job chain.
            if !(family == "moderate" && m >= 1_000) {
                methods.retain(|&name| name != "csr_C");
            }
            methods.retain(|&name| name != "csr_budget" && name != "csr_tight");
        }
        // Keep the large constrained global benchmark bounded; its smaller
        // counterparts establish the quadratic trend. No skipped result is timed.
        if n > 10_000 && pattern != "high" && family == "components" {
            methods.retain(|&s| s != "guarded");
        }
        if let Some(filter) = &method_filter {
            methods.retain(|name| filter.split(',').any(|m| m == *name));
        }
        if methods.is_empty() {
            continue;
        }
        let expected_mask = run("production", &jobs, &profile).mask;
        let expected = candidates::verify(&jobs, &profile, &expected_mask);
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
        let oracle = candidates::brute_force(tiny, &tiny_profile);
        for &method in &methods {
            if method == "packed" || method == "indirect" {
                continue;
            }
            assert_eq!(
                candidates::verify(tiny, &profile, &run(method, tiny, &profile).mask),
                oracle
            );
        }
        let mut memory = std::collections::BTreeMap::new();
        for &method in &methods {
            let (result, bytes, allocs) = allocations::measure(|| run(method, &jobs, &profile));
            if method != "packed" && method != "indirect" {
                assert_eq!(
                    candidates::verify(&jobs, &profile, &result.mask),
                    expected,
                    "{method}: {family}/{pattern}"
                );
            }
            memory.insert(method, (bytes, allocs));
        }
        for sample in 0..samples {
            shuffle(&mut methods, &mut state);
            for &method in &methods {
                let start = Instant::now();
                let result = black_box(run(method, black_box(&jobs), black_box(&profile)));
                let ns = start.elapsed().as_nanos();
                if method != "packed" && method != "indirect" {
                    assert_eq!(candidates::verify(&jobs, &profile, &result.mask), expected);
                }
                let (bytes, allocs) = memory[method];
                println!(
                    "{family},{pattern},{order},{n},{m},{k},{expected},{method},{sample},{ns},{},{},{},{},{},{},{},{},{},{bytes},{allocs}",
                    result.normalization_ns,
                    result.preprocessing_ns,
                    result.timeline_ns,
                    result.solver_ns,
                    result.reconstruction_ns,
                    result.vertices,
                    result.edges,
                    result.augmentations,
                    result.components
                );
            }
        }
        eprintln!("verified/measured n={n} m={m} k={k} {family}/{pattern}/{order}");
    }
}

#[path = "../src/capacity.rs"]
#[allow(dead_code)]
mod capacity;
#[path = "../src/capacity_profile.rs"]
#[allow(dead_code)]
mod capacity_profile;
use intervals_core::{IntervalError, max_weight_non_overlapping, max_weight_with_capacity};

fn csr_component(
    rows: &[capacity::Row<i64>],
    profile: &[capacity_profile::Segment<i64>],
) -> Measurement {
    let begin = Instant::now();
    let timeline = capacity_profile::Timeline::new(rows, profile);
    let mut flow = capacity_profile::Network::new_transshipment(&timeline, rows);
    let mut result = Measurement {
        timeline_ns: begin.elapsed().as_nanos(),
        vertices: flow.vertices(),
        edges: flow.edge_count(),
        components: 1,
        ..Measurement::default()
    };
    let begin = Instant::now();
    result.augmentations = flow.solve().unwrap();
    result.solver_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    let mut local = rows.to_vec();
    for (index, row) in local.iter_mut().enumerate() {
        row.index = index;
    }
    result.mask = vec![false; rows.len()];
    flow.reconstruct(&local, &mut result.mask);
    result.reconstruction_ns = begin.elapsed().as_nanos();
    result
}
fn run_csr(name: &str, jobs: &[Job], profile: &[Segment]) -> Measurement {
    let s: Vec<_> = jobs.iter().map(|r| r.0).collect();
    let e: Vec<_> = jobs.iter().map(|r| r.1).collect();
    let w: Vec<_> = jobs.iter().map(|r| r.2).collect();
    let ps: Vec<_> = profile.iter().map(|r| r.0).collect();
    let pe: Vec<_> = profile.iter().map(|r| r.1).collect();
    let c: Vec<_> = profile.iter().map(|r| r.2 as u64).collect();
    let begin = Instant::now();
    let normalized = capacity_profile::normalize(&ps, &pe, &c, jobs.len()).unwrap();
    let mut result = Measurement {
        normalization_ns: begin.elapsed().as_nanos(),
        ..Measurement::default()
    };
    let begin = Instant::now();
    let capacity::Prepared { mut rows, mask } = capacity::prepare(&s, &e, &w, usize::MAX).unwrap();
    result.mask = mask;
    result.preprocessing_ns = begin.elapsed().as_nanos();
    if rows.is_empty() {
        return result;
    }
    let begin = Instant::now();
    let mut timeline = capacity_profile::Timeline::new(&rows, &normalized);
    result.timeline_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    if name == "csr_tight" {
        tighten(&mut timeline);
    }
    if name == "csr_prefilter" {
        capacity_profile::filter_impossible(&mut rows, &mut timeline);
    }
    if name == "csr_guarded" && capacity_profile::all_feasible(&timeline) {
        for row in &rows {
            result.mask[row.index] = true;
        }
        result.preprocessing_ns += begin.elapsed().as_nanos();
        return result;
    }
    let components: Vec<_> = if name == "csr_components" || name == "csr_parallel" {
        capacity::components(&rows)
    } else {
        std::iter::once(0..rows.len()).collect()
    };
    result.preprocessing_ns += begin.elapsed().as_nanos();
    let solved = if name == "csr_parallel" && components.len() >= 8 {
        std::thread::scope(|scope| {
            let handles: Vec<_> = components
                .chunks(components.len().div_ceil(8))
                .map(|batch| {
                    let rows = &rows;
                    let normalized = &normalized;
                    scope.spawn(move || {
                        batch
                            .iter()
                            .map(|r| csr_component(&rows[r.clone()], normalized))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        })
    } else if components.len() == 1 {
        let begin = Instant::now();
        let mut flow = match name {
            "csr_C" => capacity_profile::Network::new_circulation(&timeline, &rows),
            "csr_adaptive" => capacity_profile::Network::new(&timeline, &rows),
            "csr_budget" | "csr_tight" => budget_network(&timeline, &rows),
            _ => capacity_profile::Network::new_transshipment(&timeline, &rows),
        };
        result.timeline_ns += begin.elapsed().as_nanos();
        result.vertices = flow.vertices();
        result.edges = flow.edge_count();
        result.components = 1;
        let begin = Instant::now();
        result.augmentations = flow.solve().unwrap();
        result.solver_ns = begin.elapsed().as_nanos();
        let begin = Instant::now();
        flow.reconstruct(&rows, &mut result.mask);
        result.reconstruction_ns = begin.elapsed().as_nanos();
        return result;
    } else {
        components
            .iter()
            .map(|r| csr_component(&rows[r.clone()], &normalized))
            .collect()
    };
    for (range, solved) in components.iter().zip(solved) {
        result.timeline_ns += solved.timeline_ns;
        result.solver_ns += solved.solver_ns;
        result.reconstruction_ns += solved.reconstruction_ns;
        result.vertices += solved.vertices;
        result.edges += solved.edges;
        result.augmentations += solved.augmentations;
        result.components += solved.components;
        for (row, selected) in rows[range.clone()].iter().zip(solved.mask) {
            result.mask[row.index] = selected;
        }
    }
    result
}

// Candidate-aware capacity bounds. A selected concurrency can increase only
// at candidate starts and decrease only at candidate ends. Both sweeps preserve
// every feasible subset and cap large unused resource changes before dispatch.
fn tighten(timeline: &mut capacity_profile::Timeline<i64>) {
    let original = timeline.capacities.clone();
    let supply = |capacities: &[usize]| {
        capacities
            .iter()
            .scan(0usize, |previous, &c| {
                let delta = c.saturating_sub(*previous);
                *previous = c;
                Some(delta)
            })
            .sum::<usize>()
    };
    let before = supply(&original);
    let mut starts = vec![0usize; timeline.endpoints.len()];
    let mut ends = vec![0usize; timeline.endpoints.len()];
    for &(s, e) in &timeline.jobs {
        starts[s] += 1;
        ends[e] += 1;
    }
    let mut bound = 0;
    for (j, c) in timeline.capacities.iter_mut().enumerate() {
        bound = (*c).min(bound + starts[j]);
        *c = bound;
    }
    bound = 0;
    for (j, c) in timeline.capacities.iter_mut().enumerate().rev() {
        bound = (*c).min(bound + ends[j + 1]);
        *c = bound;
    }
    if supply(&timeline.capacities) >= before {
        timeline.capacities = original;
    }
}

fn budget_network(
    timeline: &capacity_profile::Timeline<i64>,
    rows: &[capacity::Row<i64>],
) -> capacity_profile::Network {
    let mut previous = 0;
    let a: usize = timeline
        .capacities
        .iter()
        .map(|&c| {
            let d = c.saturating_sub(previous);
            previous = c;
            d
        })
        .sum();
    let mut balances = vec![0i64; timeline.endpoints.len()];
    for &(s, e) in &timeline.jobs {
        balances[s] -= 1;
        balances[e] += 1;
    }
    let c: usize = balances.into_iter().map(|b| b.max(0) as usize).sum();
    if a > 4 * c {
        capacity_profile::Network::new_circulation(timeline, rows)
    } else {
        capacity_profile::Network::new_transshipment(timeline, rows)
    }
}
