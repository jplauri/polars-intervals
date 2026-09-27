#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/coverage.rs"]
mod candidates;
#[path = "../tests/support/coverage.rs"]
mod oracle;

use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn data(family: &str, n: usize) -> Vec<(i64, i64)> {
    let mut state = 20260927;
    (0..n)
        .map(|i| {
            let x = i as i64;
            let size = n as i64;
            match family {
                "disjoint" => (x * 100, x * 100 + 1 + x % 97),
                "identical" => (0, 100),
                "nested" => (x, 2 * size - x),
                "dense" => {
                    let a = (random(&mut state) % (n as u64 + 1)) as i64;
                    (a, a + size / 2 + (random(&mut state) % 100) as i64 + 1)
                }
                "sparse" => {
                    let a = (random(&mut state) % (n as u64 * 20 + 1)) as i64;
                    (a, a + (random(&mut state) % 50) as i64 + 1)
                }
                "touching" => (x * 5, x * 5 + 5),
                "staircase" => (x * 3, x * 3 + 17),
                "equal_starts" => (x / 32 * 100, x / 32 * 100 + 1 + x % 32),
                "equal_ends" => (x / 32 * 100 + x % 32, x / 32 * 100 + 40),
                "duplicates" => (x / 16 * 7, x / 16 * 7 + 30),
                "empty" => (x * 3, x * 3 + if i % 10 == 0 { 20 } else { 0 }),
                "dominated" => (x / 32 * 100 + x % 32, x / 32 * 100 + 70 - x % 32),
                "components" => (x / 16 * 1000 + x % 16 * 2, x / 16 * 1000 + x % 16 * 2 + 7),
                "redundant_long" => (x, 10 * size + x),
                "complementary" => {
                    if i < n / 2 {
                        (0, size)
                    } else {
                        (x * 3, x * 3 + size / 3 + 1)
                    }
                }
                // OPT(1..4) = 10,18,21,23; the unique geometry chosen for k=1
                // disappears at k=2 and returns at k=4. Repetition scales input n.
                "budget_sensitive" => [(0, 10), (-5, 4), (6, 15), (30, 33)][i % 4],
                "saturation" => {
                    if i == 0 {
                        (0, 2 * size)
                    } else if i == 1 {
                        (2 * size, 4 * size)
                    } else {
                        (x, 3 * size + x)
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect()
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "benchmarks require release mode");
    // Tiny exhaustive gate before any timing rows are emitted.
    for family in ["dense", "sparse", "nested", "components", "saturation"] {
        let (s, e): (Vec<_>, Vec<_>) = data(family, 10).into_iter().unzip();
        for k in [0, 1, 2, 4, 10] {
            let best = oracle::brute(&s, &e, k);
            assert_eq!(oracle::quadratic(&s, &e, k), best);
            for method in candidates::METHODS {
                assert_eq!(
                    oracle::objective(&s, &e, &candidates::run(&s, &e, k, method).mask),
                    best
                );
            }
        }
    }
    println!(
        "family,order,n,k,method,sample,total_ns,sort_ns,preprocess_ns,dp_ns,reconstruct_ns,peak_bytes,allocations,measure,count"
    );
    let families = [
        "disjoint",
        "identical",
        "nested",
        "dense",
        "sparse",
        "touching",
        "staircase",
        "equal_starts",
        "equal_ends",
        "duplicates",
        "empty",
        "dominated",
        "components",
        "redundant_long",
        "complementary",
        "budget_sensitive",
        "saturation",
    ];
    for n in [0, 8, 1_000, 10_000, 100_000, 1_000_000] {
        for family in families {
            if n == 0 && family != "empty" {
                continue;
            }
            if n == 8 && family != "staircase" {
                continue;
            }
            // Large cases deliberately cover both retained-size extremes and
            // input order. The smaller matrix covers every structure/candidate.
            if n == 1_000_000
                && !["disjoint", "dense", "staircase", "dominated", "components"].contains(&family)
            {
                continue;
            }
            for order in ["sorted", "reverse", "shuffled"] {
                if n < 1_000 && order != "sorted" {
                    continue;
                }
                let mut rows = data(family, n);
                rows.sort_unstable_by_key(|&(s, e)| (e, s));
                if order == "reverse" {
                    rows.reverse();
                }
                if order == "shuffled" {
                    let mut state = 20260927;
                    for i in (1..n).rev() {
                        let j = random(&mut state) as usize % (i + 1);
                        rows.swap(i, j);
                    }
                }
                let (s, e): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
                let budgets: &[usize] = if n == 1_000 {
                    &[0, 1, 2, 3, 4, 8, 16, 32, 64, 1000]
                } else if n == 1_000_000 {
                    &[0, 1, 8, 64]
                } else {
                    &[0, 1, 2, 3, 4, 8, 16, 32, 64]
                };
                for &k in budgets {
                    let reference = candidates::run(&s, &e, k, "rolling");
                    let expected = oracle::objective(&s, &e, &reference.mask);
                    assert_eq!(expected, (reference.score.0, reference.score.1.0));
                    drop(reference);
                    for &method in candidates::METHODS {
                        // Bound full tables to ~256 MB; replay to 30M state
                        // updates; convolution to manageable component counts.
                        if ["full", "unpruned"].contains(&method) && n * k > 4_000_000 {
                            continue;
                        }
                        if method == "recompute" && n * k * k > 30_000_000 {
                            continue;
                        }
                        if method == "components" && (n > 10_000 || k > 64) {
                            continue;
                        }
                        let checked = candidates::run(&s, &e, k, method);
                        assert_eq!(
                            oracle::objective(&s, &e, &checked.mask),
                            expected,
                            "{family}/{order}/{n}/{k}/{method}"
                        );
                        if method != "production" {
                            assert_eq!((checked.score.0, checked.score.1.0), expected);
                        }
                        drop(checked);
                        let (memory, peak, allocs) =
                            allocations::measure(|| candidates::run(&s, &e, k, method));
                        assert_eq!(oracle::objective(&s, &e, &memory.mask), expected);
                        drop(memory);
                        for sample in 0..3 {
                            let begin = Instant::now();
                            let result =
                                black_box(candidates::run(black_box(&s), black_box(&e), k, method));
                            let total = begin.elapsed().as_nanos();
                            println!(
                                "{family},{order},{n},{k},{method},{sample},{total},{},{},{},{},{peak},{allocs},{},{}",
                                result.sort.as_nanos(),
                                result.preprocess.as_nanos(),
                                result.dp.as_nanos(),
                                result.reconstruct.as_nanos(),
                                expected.0,
                                expected.1
                            );
                        }
                    }
                }
            }
        }
    }
}
