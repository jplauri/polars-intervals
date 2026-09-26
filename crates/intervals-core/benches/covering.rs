//! Run with cargo bench -p intervals-core --bench covering --locked.
use intervals_core::IntervalError;
use std::hint::black_box;
use std::time::Instant;

#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/covering.rs"]
mod candidates;
#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}
fn shuffle<T>(values: &mut [T], seed: &mut u64) {
    for i in (1..values.len()).rev() {
        values.swap(i, random(seed) as usize % (i + 1));
    }
}

fn workload(
    n: usize,
    family: &str,
    distribution: &str,
    seed: &mut u64,
) -> (Vec<(i64, i64, i128)>, i64) {
    let end = n as i64;
    let mut rows: Vec<_> = (0..n)
        .map(|i| {
            let i = i as i64;
            let (s, e) = match family {
                "single" => (0, end),
                "two" | "duplicates" => {
                    if i % 2 == 0 {
                        (0, end / 2)
                    } else {
                        (end / 2, end)
                    }
                }
                "disjoint" => (i * 2, i * 2 + 1),
                "chain" | "fail_begin" | "fail_middle" | "fail_end" | "expensive_long" => {
                    (i, i + 1)
                }
                "dense" => (i.saturating_sub(end / 4).max(0), (i + end / 4).min(end)),
                "nested" => (i, 2 * end - i),
                "equal_starts" => (0, i + 1),
                "equal_ends" => (i, end),
                "outside" => (-end - i, end + i),
                "irrelevant" => (end + i + 1, end + i + 2),
                "empty" => (i, i),
                "giant_component" => (i, (i + 3).min(end)),
                _ => unreachable!(),
            };
            let cost = match distribution {
                "equal" | "expensive" => 1,
                "zero" => i128::from(random(seed).is_multiple_of(4)),
                "random" => (random(seed) % 21) as i128,
                "skewed" => {
                    if i % 32 == 0 {
                        1_000_000
                    } else {
                        1
                    }
                }
                "different_duplicates" => (i % 11) as i128,
                "ties" => (i % 2) as i128,
                _ => unreachable!(),
            };
            (s, e, cost)
        })
        .collect();
    match family {
        "irrelevant" | "empty" => rows[n / 2] = (0, end, 1),
        "fail_begin" => rows[0] = (1, 1, 0),
        "fail_middle" => rows[n / 2] = (end / 2, end / 2, 0),
        "fail_end" => rows[n - 1] = (end, end, 0),
        "expensive_long" => {
            rows[n - 1] = (0, end, 1_000_000);
            rows[n - 2].1 = end;
        }
        _ => (),
    }
    (rows, end)
}

fn verify<T: Ord + Copy>(
    s: &[T],
    e: &[T],
    w: &[i128],
    left: T,
    right: T,
    result: &Result<Vec<bool>, IntervalError>,
) -> Option<(i128, usize)> {
    match result {
        Ok(mask) => {
            assert_eq!(mask.len(), s.len());
            assert!(candidates::covers(s, e, mask, left, right));
            Some(candidates::objective(w, mask))
        }
        Err(error) => {
            assert_eq!(*error, IntervalError::InfeasibleCover);
            None
        }
    }
}

fn main() {
    let maximum = std::env::var("COVER_BENCH_MAX")
        .ok()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(1_000_000);
    let samples = std::env::var("COVER_BENCH_SAMPLES")
        .ok()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(3);
    let mut seed = 20260926;
    println!(
        "family,order,costs,n,method,sample,total_ns,preprocessing_ns,optimization_ns,reconstruction_ns,peak_bytes,allocations"
    );
    for n in [1_000, 10_000, 100_000, 1_000_000]
        .into_iter()
        .filter(|&n| n <= maximum)
    {
        for family in [
            "single",
            "two",
            "disjoint",
            "chain",
            "dense",
            "nested",
            "duplicates",
            "equal_starts",
            "equal_ends",
            "outside",
            "irrelevant",
            "empty",
            "giant_component",
            "fail_begin",
            "fail_middle",
            "fail_end",
            "expensive_long",
        ] {
            let distributions: &[&str] = if family == "dense" || family == "duplicates" {
                &[
                    "equal",
                    "zero",
                    "random",
                    "skewed",
                    "different_duplicates",
                    "expensive",
                    "ties",
                ]
            } else if family == "expensive_long" {
                &["expensive"]
            } else {
                &["random"]
            };
            for &distribution in distributions {
                let (original, right) = workload(n, family, distribution, &mut seed);
                for order in ["sorted", "nearly_sorted", "shuffled"] {
                    let mut rows = original.clone();
                    rows.sort_unstable_by_key(|r| r.0);
                    match order {
                        "nearly_sorted" => {
                            for _ in 0..(n / 100).max(1) {
                                let i = random(&mut seed) as usize % (n - 1);
                                rows.swap(i, i + 1);
                            }
                        }
                        "shuffled" => shuffle(&mut rows, &mut seed),
                        _ => (),
                    }
                    let s: Vec<_> = rows.iter().map(|r| r.0).collect();
                    let e: Vec<_> = rows.iter().map(|r| r.1).collect();
                    let w: Vec<_> = rows.iter().map(|r| r.2).collect();
                    measure(
                        &s,
                        &e,
                        &w,
                        (0, right),
                        &format!("{family},{order},{distribution}"),
                        samples,
                        &mut seed,
                    );
                }
            }
            eprintln!("verified n={n} family={family}");
        }
    }
    physical::<i32>("date32", maximum, samples);
    physical::<i64>("datetime64", maximum, samples);
}

// Temporal physical widths, without introducing a temporal dependency into core.
// The companion release-wheel benchmark measures actual logical Polars dtypes.
fn physical<T: Ord + Copy + TryFrom<i64>>(label: &str, maximum: usize, samples: usize) {
    let mut seed = 20260926;
    for n in [1_000, 10_000, 100_000, 1_000_000]
        .into_iter()
        .filter(|&n| n <= maximum)
    {
        for family in ["chain", "dense", "duplicates"] {
            let (mut rows, right) = workload(n, family, "random", &mut seed);
            rows.sort_unstable_by_key(|r| r.0);
            for order in ["sorted", "shuffled"] {
                if order == "shuffled" {
                    shuffle(&mut rows, &mut seed);
                }
                let convert = |x| T::try_from(x).ok().unwrap();
                let s: Vec<_> = rows.iter().map(|r| convert(r.0)).collect();
                let e: Vec<_> = rows.iter().map(|r| convert(r.1)).collect();
                let w: Vec<_> = rows.iter().map(|r| r.2).collect();
                measure(
                    &s,
                    &e,
                    &w,
                    (convert(0), convert(right)),
                    &format!("{label}_{family},{order},random"),
                    samples,
                    &mut seed,
                );
            }
        }
        eprintln!("verified {label} n={n}");
    }
}

// One measurement path keeps warmups, verification and timing boundaries identical
// across all endpoint widths. Inputs and the CSV case label are built outside it.
fn measure<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    costs: &[i128],
    (left, right): (T, T),
    case: &str,
    samples: usize,
    seed: &mut u64,
) {
    let n = starts.len();
    let unit = vec![1; n];
    let expected_mc = verify(
        starts,
        ends,
        &unit,
        left,
        right,
        &intervals_core::minimum_cover(starts, ends, left, right),
    );
    let expected_mcc = verify(
        starts,
        ends,
        costs,
        left,
        right,
        &intervals_core::minimum_cost_cover(starts, ends, costs, left, right),
    );
    let mut methods = vec!["MC-A", "MC-B", "MC-C", "MC-A-detect", "MCC-A", "MCC-B"];
    if n <= 1_000 {
        methods.push("MCC-C");
    }
    let run = |method: &str| {
        if method.starts_with("MCC") {
            candidates::weighted(
                black_box(starts),
                black_box(ends),
                black_box(costs),
                left,
                right,
                method,
            )
        } else {
            candidates::cardinality(black_box(starts), black_box(ends), left, right, method)
        }
    };
    let check = |method: &str, result: &Result<Vec<bool>, IntervalError>| {
        let (weights, expected) = if method.starts_with("MCC") {
            (costs, expected_mcc)
        } else {
            (unit.as_slice(), expected_mc)
        };
        assert_eq!(verify(starts, ends, weights, left, right, result), expected);
    };
    let mut memory = std::collections::BTreeMap::new();
    for &method in &methods {
        let (result, peak, allocs) = allocations::measure(|| run(method));
        check(method, &result.result);
        memory.insert(method, (peak, allocs));
    }
    // Allocation pass warms each candidate; alternate timed execution order.
    for sample in 0..samples {
        shuffle(&mut methods, seed);
        for &method in &methods {
            let begin = Instant::now();
            let result = black_box(run(method));
            let total = begin.elapsed().as_nanos();
            check(method, &result.result);
            let (peak, allocs) = memory[method];
            println!(
                "{case},{n},{method},{sample},{total},{},{},{},{peak},{allocs}",
                result.preprocessing_ns, result.optimization_ns, result.reconstruction_ns
            );
        }
    }
}
