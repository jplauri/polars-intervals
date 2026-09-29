//! Run with cargo bench -p intervals-core --bench assign_lanes --locked.
mod support;

use std::hint::black_box;
use std::time::Instant;
use support::{optimum, shuffle, verify};

fn main() {
    if cfg!(debug_assertions) {
        panic!("run this benchmark in release mode");
    }
    println!("family,order,n,omega,algorithm,sample,ns");
    let mut seed = 42;
    for n in [1_000usize, 10_000, 100_000, 1_000_000] {
        for family in [
            "disjoint",
            "low8",
            "moderate128",
            "clique",
            "nested",
            "staircase",
            "ties",
            "duplicates",
            "mixed_empty",
        ] {
            let mut intervals: Vec<_> = (0..n as i64)
                .map(|i| match family {
                    "disjoint" => (i, i + 1),
                    "low8" => (i, i + 8),
                    "moderate128" => (i, i + 128),
                    "clique" => (i, i + n as i64),
                    "nested" => (i, 2 * n as i64 - i),
                    "staircase" => (i, i + 2),
                    "ties" => (i / 32 * 16, i / 32 * 16 + 16 + i % 4),
                    "duplicates" => (i / 16 * 4, i / 16 * 4 + 8),
                    "mixed_empty" => (i / 4, i / 4 + if i % 3 == 0 { 0 } else { 8 }),
                    _ => unreachable!(),
                })
                .collect();
            for order in ["sorted", "shuffled"] {
                if order == "shuffled" {
                    shuffle(&mut intervals, &mut seed);
                }
                let (starts, ends): (Vec<_>, Vec<_>) = intervals.iter().copied().unzip();
                let expected = optimum(&starts, &ends);
                for sample in 0..11 {
                    let begin = Instant::now();
                    let lanes = black_box(
                        intervals_core::assign_lanes(black_box(&starts), black_box(&ends)).unwrap(),
                    );
                    let elapsed = begin.elapsed().as_nanos();
                    // Every measured output is independently checked, outside timing.
                    verify(&starts, &ends, &lanes, expected);
                    if sample >= 2 {
                        println!(
                            "{family},{order},{n},{expected},production,{},{elapsed}",
                            sample - 2
                        );
                    }
                }
            }
        }
    }
}
