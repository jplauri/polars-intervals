//! Compare the production function before/after shortening coordinate-buffer lifetime.
extern crate self as intervals_core;

#[allow(dead_code)]
#[path = "../../crates/intervals-core/src/lib.rs"]
mod core_types;
pub use core_types::*;

#[path = "containment-before.rs"]
mod before;
#[path = "../../crates/intervals-core/src/containment.rs"]
mod after;
#[allow(dead_code)]
#[path = "../../crates/intervals-core/benches/support/containment.rs"]
mod reference;
#[path = "../../crates/intervals-core/benches/support/allocations.rs"]
mod allocations;

use std::fmt::Debug;
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn measure<T: Ord + Copy + Debug>(
    starts: &[T],
    ends: &[T],
    dtype: &str,
    scenario: &str,
    file: &mut impl Write,
    seed: &mut u64,
) -> std::io::Result<()> {
    type Run<T> = fn(&[T], &[T]) -> Result<Vec<usize>, IntervalError>;
    let runs: [(&str, Run<T>); 2] = [
        ("before", before::containment_counts::<T>),
        ("after", after::containment_counts::<T>),
    ];
    let expected = before::containment_counts(starts, ends).unwrap();
    let memory = runs.map(|(_, run)| {
        let (counts, peak, allocations) = allocations::measure(|| run(starts, ends).unwrap());
        assert_eq!(counts, expected);
        (peak, allocations)
    });
    for sample in 0..11 {
        let first = (reference::random(seed) % 2) as usize;
        for index in [first, 1 - first] {
            let (name, run) = runs[index];
            let begin = Instant::now();
            let counts = black_box(run(black_box(starts), black_box(ends)).unwrap());
            let elapsed = begin.elapsed().as_nanos();
            assert_eq!(counts, expected);
            if sample >= 2 {
                let (peak, allocations) = memory[index];
                writeln!(
                    file,
                    "{dtype},{scenario},{},{name},{},{elapsed},{peak},{allocations}",
                    starts.len(),
                    sample - 2,
                )?;
            }
        }
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    assert!(!cfg!(debug_assertions), "measure an optimized build");
    let path = std::env::args().nth(1).expect("CSV output path");
    let mut file = BufWriter::new(std::fs::File::create(path)?);
    writeln!(file, "dtype,scenario,n,variant,sample,total_ns,peak_bytes,allocations")?;
    let mut seed = 20260927;
    for scenario in reference::SCENARIOS {
        let (starts, ends) = reference::dataset(scenario, 100);
        let expected = reference::naive(&starts, &ends);
        assert_eq!(before::containment_counts(&starts, &ends).unwrap(), expected);
        assert_eq!(after::containment_counts(&starts, &ends).unwrap(), expected);
        for n in [1_000, 100_000, 1_000_000] {
            let (starts, ends) = reference::dataset(scenario, n);
            measure(&starts, &ends, "i64", scenario, &mut file, &mut seed)?;
            let starts: Vec<_> = starts.into_iter().map(|x| i32::try_from(x).unwrap()).collect();
            let ends: Vec<_> = ends.into_iter().map(|x| i32::try_from(x).unwrap()).collect();
            measure(&starts, &ends, "i32", scenario, &mut file, &mut seed)?;
            file.flush()?;
        }
        eprintln!("verified and measured {scenario}");
    }
    file.flush()
}
