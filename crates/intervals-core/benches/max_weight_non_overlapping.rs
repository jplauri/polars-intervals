//! cargo bench -p intervals-core --bench max_weight_non_overlapping --locked
#[path = "support/weighted.rs"]
mod weighted;

use std::hint::black_box;
use std::time::Instant;
use weighted::{CANDIDATES, random, shuffle, suffix_optimum, verify};

fn main() {
    if cfg!(debug_assertions) {
        panic!("run this benchmark in release mode");
    }
    println!(
        "family,order,weights,n,objective,algorithm,sample,ns,preprocessing_ns,optimization_ns,reconstruction_ns,peak_buffer_bytes,buffer_allocations"
    );
    let mut seed = 42;
    // Optional maximum size allows a fast reproducibility smoke run.
    let max_n = std::env::var("WEIGHTED_BENCH_MAX_N")
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(1_000_000);
    for n in [1_000usize, 10_000, 100_000, 1_000_000]
        .into_iter()
        .filter(|&n| n <= max_n)
    {
        for family in [
            "disjoint",
            "low8",
            "moderate128",
            "dense",
            "nested",
            "staircase",
            "duplicates",
            "equal_endpoints",
            "touching",
            "empty",
            "random_lengths",
        ] {
            for distribution in ["positive", "mixed", "zeros", "ties", "wide", "expensive"] {
                let mut intervals: Vec<_> = (0..n as i64)
                    .map(|i| {
                        let (s, e) = match family {
                            "disjoint" => (3 * i, 3 * i + 2),
                            "low8" => (i, i + 8),
                            "moderate128" => (i, i + 128),
                            "dense" => (i, i + n as i64),
                            "nested" => (i, 2 * n as i64 - i),
                            "staircase" => (i, 2 * i + 2),
                            "duplicates" => (i / 16 * 4, i / 16 * 4 + 8),
                            "equal_endpoints" => (i / 32 * 16, i / 32 * 16 + 16 + i % 4),
                            "touching" => (i, i + 1),
                            "empty" => (i / 4, i / 4 + if i % 3 != 0 { 0 } else { 8 }),
                            "random_lengths" => {
                                (i, i + 1 + (random(&mut seed) % (n as u64 / 8)) as i64)
                            }
                            _ => unreachable!(),
                        };
                        let r = random(&mut seed);
                        let weight = match distribution {
                            "positive" => 1 + (r % 100) as i64,
                            "mixed" => (r % 201) as i64 - 100,
                            "zeros" => {
                                if r.is_multiple_of(5) {
                                    1 + (r % 100) as i64
                                } else {
                                    0
                                }
                            }
                            "ties" => 1 + (r % 3) as i64,
                            "wide" => 1i64 << (r % 63),
                            "expensive" => 10,
                            _ => unreachable!(),
                        };
                        (s, e, weight)
                    })
                    .collect();
                if distribution == "expensive" {
                    // One long expensive row competes against every ordinary row.
                    let end = intervals.iter().map(|r| r.1).max().unwrap();
                    intervals[0] = (0, end, 5 * n as i64);
                }
                intervals.sort_unstable_by_key(|&(s, e, _)| (e, s));
                for order in ["finish_sorted", "shuffled"] {
                    if order == "shuffled" {
                        shuffle(&mut intervals, &mut seed);
                    }
                    let s: Vec<_> = intervals.iter().map(|r| r.0).collect();
                    let e: Vec<_> = intervals.iter().map(|r| r.1).collect();
                    let w: Vec<_> = intervals.iter().map(|r| r.2).collect();
                    let expected = suffix_optimum(&s, &e, &w);
                    // Independently validate every candidate before any timing is accepted.
                    for (_, run) in CANDIDATES {
                        let result = run(&s, &e, &w).unwrap();
                        assert_eq!(verify(&s, &e, &w, &result.mask), expected);
                    }
                    let production =
                        intervals_core::max_weight_non_overlapping(&s, &e, &w).unwrap();
                    assert_eq!(verify(&s, &e, &w, &production), expected);
                    let mut methods = CANDIDATES;
                    for sample in 0..7 {
                        shuffle(&mut methods, &mut seed);
                        for (name, run) in methods {
                            let begin = Instant::now();
                            let result = black_box(
                                run(black_box(&s), black_box(&e), black_box(&w)).unwrap(),
                            );
                            let ns = begin.elapsed().as_nanos();
                            assert_eq!(verify(&s, &e, &w, &result.mask), expected);
                            if sample >= 2 {
                                println!(
                                    "{family},{order},{distribution},{n},{expected},{name},{},{ns},{},{},{},{},{}",
                                    sample - 2,
                                    result.preprocessing_ns,
                                    result.optimization_ns,
                                    result.reconstruction_ns,
                                    result.peak_buffer_bytes,
                                    result.buffer_allocations
                                );
                            }
                        }
                    }
                }
            }
        }
        eprintln!("verified and measured n={n}");
    }
}
