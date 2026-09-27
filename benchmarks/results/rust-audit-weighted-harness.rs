extern crate self as intervals_core;

#[path = "../../crates/intervals-core/src/lib.rs"]
mod core;
pub use core::*;
#[path = "../../crates/intervals-core/benches/support/allocations.rs"]
mod allocations;
#[path = "before.rs"]
mod before;
#[path = "../../crates/intervals-core/benches/support/weighted.rs"]
mod reference;

use std::hint::black_box;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

type Solver = fn(&[i64], &[i64], &[i64]) -> Result<Vec<bool>, IntervalError>;

fn main() {
    assert!(!cfg!(debug_assertions));
    println!(
        "family,order,weights,n,objective,algorithm,sample,ns,peak_allocated_bytes,allocations"
    );
    // Fixtures and independent oracle match the existing weighted benchmark.
    let mut seed = 42;
    for n in [1_000usize, 100_000, 1_000_000] {
        for family in ["disjoint", "moderate128", "random_lengths"] {
            for distribution in ["positive", "mixed"] {
                let mut rows: Vec<_> = (0..n as i64)
                    .map(|i| {
                        let (s, e) = match family {
                            "disjoint" => (3 * i, 3 * i + 2),
                            "moderate128" => (i, i + 128),
                            "random_lengths" => (
                                i,
                                i + 1 + (reference::random(&mut seed) % (n as u64 / 8)) as i64,
                            ),
                            _ => unreachable!(),
                        };
                        let r = reference::random(&mut seed);
                        let w = if distribution == "positive" {
                            1 + (r % 100) as i64
                        } else {
                            (r % 201) as i64 - 100
                        };
                        (s, e, w)
                    })
                    .collect();
                rows.sort_unstable_by_key(|&(s, e, _)| (e, s));
                for order in ["finish_sorted", "shuffled"] {
                    if order == "shuffled" {
                        reference::shuffle(&mut rows, &mut seed);
                    }
                    let s: Vec<_> = rows.iter().map(|r| r.0).collect();
                    let e: Vec<_> = rows.iter().map(|r| r.1).collect();
                    let w: Vec<_> = rows.iter().map(|r| r.2).collect();
                    let expected = reference::suffix_optimum(&s, &e, &w);
                    let mut methods: [(&str, Solver, usize, usize); 2] = [
                        ("before", before::max_weight_non_overlapping, 0, 0),
                        ("after", max_weight_non_overlapping, 0, 0),
                    ];
                    for (_, run, peak, count) in &mut methods {
                        let (mask, measured_peak, measured_count) =
                            allocations::measure(|| run(&s, &e, &w).unwrap());
                        assert_eq!(reference::verify(&s, &e, &w, &mask), expected);
                        *peak = measured_peak;
                        *count = measured_count;
                    }
                    for sample in 0..7 {
                        reference::shuffle(&mut methods, &mut seed);
                        for (name, run, peak, count) in methods {
                            let begin = Instant::now();
                            let mask = black_box(
                                run(black_box(&s), black_box(&e), black_box(&w)).unwrap(),
                            );
                            let ns = begin.elapsed().as_nanos();
                            assert_eq!(reference::verify(&s, &e, &w, &mask), expected);
                            if sample >= 2 {
                                println!(
                                    "{family},{order},{distribution},{n},{expected},{name},{},{ns},{peak},{count}",
                                    sample - 2
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
