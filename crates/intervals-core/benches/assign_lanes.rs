//! Run with cargo bench -p intervals-core --bench assign_lanes --locked.
mod support;

use std::hint::black_box;
use std::time::Instant;
use support::{CANDIDATES, optimum, shuffle, verify};

fn main() {
    if cfg!(debug_assertions) {
        panic!("run this benchmark in release mode");
    }
    println!("family,order,n,omega,algorithm,sample,ns,buffer_bytes");
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
                // Validate every candidate on exactly these inputs before timing any.
                for (_, run) in CANDIDATES {
                    let (lanes, _) = run(&starts, &ends).unwrap();
                    verify(&starts, &ends, &lanes, expected);
                    assert_eq!(lanes, run(&starts, &ends).unwrap().0);
                }
                assert_eq!(
                    intervals_core::assign_lanes(&starts, &ends).unwrap(),
                    support::heap(&starts, &ends).unwrap().0,
                    "production and its measured reference diverged"
                );
                let mut methods = CANDIDATES;
                for sample in 0..11 {
                    shuffle(&mut methods, &mut seed);
                    for (name, run) in methods {
                        let begin = Instant::now();
                        let (lanes, bytes) =
                            black_box(run(black_box(&starts), black_box(&ends)).unwrap());
                        let elapsed = begin.elapsed().as_nanos();
                        // Every measured output is independently checked, outside timing.
                        verify(&starts, &ends, &lanes, expected);
                        if sample >= 2 {
                            println!(
                                "{family},{order},{n},{expected},{name},{},{elapsed},{bytes}",
                                sample - 2
                            );
                        }
                    }
                }
            }
        }
    }
}
