//! Release only: cargo bench -p intervals-core --bench minimum_stabbing_points --locked
use intervals_core::{DiscreteEndpoint, minimum_stabbing_points};
use std::{fmt::Debug, hint::black_box, time::Instant};
#[path = "support/allocations.rs"]
mod allocations;
#[path = "../tests/support/stabbing.rs"]
mod oracle;
#[path = "support/random.rs"]
mod random;
use random::{random, shuffle};
#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn workload(n: usize, family: &str, seed: &mut u64) -> Vec<(i64, i64)> {
    let size = n as i64;
    (0..size)
        .map(|i| {
            let s = (random(seed) % (n as u64 * 10)) as i64;
            match family {
                "disjoint" => (3 * i, 3 * i + 2),
                "common" => (-i, size + i + 1),
                "sparse" => (s, s + 1 + (random(seed) % 8) as i64),
                "dense" => (s, s + 1 + (random(seed) % (n as u64 * 2)) as i64),
                "nested" => (i, 2 * size - i),
                "touching" => (2 * i, 2 * i + 2),
                "staircase" => (i, i + 2),
                "identical" => (1, 5),
                "equal_ends" => (i, size),
                "equal_starts" => (0, i + 1),
                "unit" => (s, s + 1),
                "wide" => (s - 1_000_000_000, s + 1_000_000_000),
                _ => unreachable!(),
            }
        })
        .collect()
}
fn measure<T: DiscreteEndpoint + Debug>(s: &[T], e: &[T], case: &str, samples: usize) {
    let optimal = oracle::packing(s, e);
    let (expected, peak, count) = allocations::measure(|| minimum_stabbing_points(s, e).unwrap());
    assert_eq!(expected.len(), optimal);
    assert!(expected.windows(2).all(|w| w[0] < w[1]));
    for (&s, &e) in s.iter().zip(e) {
        let i = expected.partition_point(|&p| p < s);
        assert!(i < expected.len() && expected[i] < e);
    }
    for sample in 0..samples {
        let begin = Instant::now();
        let points = black_box(minimum_stabbing_points(black_box(s), black_box(e)).unwrap());
        let total = begin.elapsed().as_nanos();
        assert_eq!(points, expected);
        println!(
            "{case},{},production,{sample},{total},{peak},{count},{}",
            s.len(),
            points.len()
        );
    }
}
fn main() {
    // Verify production against BOTH exponential oracles before timing.
    let mut seed = 20260926;
    for _ in 0..256 {
        let rows: Vec<_> = (0..8)
            .map(|_| {
                let s = (random(&mut seed) % 12) as i64 - 5;
                (s, s + 1 + (random(&mut seed) % (7 - s) as u64) as i64)
            })
            .collect();
        let (s, e): (Vec<_>, Vec<_>) = rows.iter().copied().unzip();
        let optimum = oracle::brute_stabbing(&rows).unwrap();
        assert_eq!(optimum, oracle::brute_packing(&rows));
        assert_eq!(optimum, oracle::packing(&s, &e));
        let points = minimum_stabbing_points(&s, &e).unwrap();
        assert_eq!(points.len(), optimum);
        assert!(
            rows.iter()
                .all(|&(s, e)| points.iter().any(|&p| s <= p && p < e))
        );
    }
    let max = std::env::var("STABBING_BENCH_MAX")
        .ok()
        .map(|v| v.parse().unwrap())
        .unwrap_or(3_000_000);
    let samples = std::env::var("STABBING_BENCH_SAMPLES")
        .ok()
        .map(|v| v.parse().unwrap())
        .unwrap_or(3);
    println!("dtype,family,order,n,method,sample,total_ns,peak_bytes,allocations,points");
    for n in [1_000, 10_000, 100_000, 1_000_000, 3_000_000]
        .into_iter()
        .filter(|&n| n <= max)
    {
        for family in [
            "disjoint",
            "common",
            "sparse",
            "dense",
            "nested",
            "touching",
            "staircase",
            "identical",
            "equal_ends",
            "equal_starts",
            "unit",
            "wide",
        ] {
            let mut rows = workload(n, family, &mut seed);
            rows.sort_unstable_by_key(|r| r.1);
            for order in ["sorted", "reverse", "shuffled"] {
                match order {
                    "reverse" => rows.reverse(),
                    "shuffled" => shuffle(&mut rows, &mut seed),
                    _ => (),
                }
                let (s, e): (Vec<_>, Vec<_>) = rows.iter().copied().unzip();
                // Datetime uses this same i64 kernel; logical adaptation is measured separately.
                measure(&s, &e, &format!("i64,{family},{order}"), samples);
                if ["disjoint", "dense", "identical"].contains(&family) {
                    let s: Vec<_> = s.iter().map(|&s| i32::try_from(s).unwrap()).collect();
                    let e: Vec<_> = e.iter().map(|&e| i32::try_from(e).unwrap()).collect();
                    measure(&s, &e, &format!("date32,{family},{order}"), samples);
                }
            }
            eprintln!("verified n={n} family={family}");
        }
    }
}
