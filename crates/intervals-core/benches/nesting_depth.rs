//! Release-only production timing. Environment: NESTING_CSV, NESTING_SIZES,
//! NESTING_SAMPLES, NESTING_SCENARIOS. Defaults run the complete matrix.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/nesting_depth.rs"]
mod support;

use std::{hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

// Each group is solved independently, like one Polars window partition.
fn grouped<T: Ord + Copy>(s: &[T], e: &[T], groups: usize) -> Vec<usize> {
    if groups == 1 {
        return intervals_core::nesting_depths(s, e).unwrap();
    }
    let mut depths = Vec::with_capacity(s.len());
    for group in 0..groups {
        let start = group * s.len() / groups;
        let end = (group + 1) * s.len() / groups;
        depths.extend(intervals_core::nesting_depths(&s[start..end], &e[start..end]).unwrap());
    }
    depths
}

fn measure<T: Ord + Copy>(
    file: &mut impl Write,
    s: &[T],
    e: &[T],
    case: &str,
    groups: usize,
    samples: usize,
) {
    let (expected, peak, count) = allocations::measure(|| grouped(s, e, groups));
    let max_depth = expected.iter().copied().max().unwrap_or(0);
    for sample in 0..samples {
        let tick = Instant::now();
        let depths = black_box(grouped(black_box(s), black_box(e), groups));
        let total = tick.elapsed().as_nanos();
        assert_eq!(depths, expected, "{case}");
        writeln!(
            file,
            "{case},{},{groups},production,{sample},{total},{peak},{count},{max_depth}",
            s.len()
        )
        .unwrap();
    }
    file.flush().unwrap();
}

// Keep debug builds compilable for linting, but never time debug kernels.
#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench (release mode)");
    let sizes: Vec<usize> = std::env::var("NESTING_SIZES")
        .unwrap_or_else(|_| "1000,10000,100000,1000000,3000000".into())
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();
    let samples = std::env::var("NESTING_SAMPLES")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(3);
    let scenarios =
        std::env::var("NESTING_SCENARIOS").unwrap_or_else(|_| support::SCENARIOS.join(","));
    let path = std::env::var("NESTING_CSV").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/nesting-depth.csv"
        )
        .into()
    });
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(
        file,
        "dtype,scenario,n,groups,method,sample,total_ns,peak_bytes,allocations,max_depth"
    )
    .unwrap();
    for scenario in scenarios.split(',') {
        let (s, e) = support::dataset(scenario, 100);
        assert_eq!(
            intervals_core::nesting_depths(&s, &e).unwrap(),
            support::naive(&s, &e),
            "{scenario}"
        );
        for &n in &sizes {
            let (s, e) = support::dataset(scenario, n);
            measure(&mut file, &s, &e, &format!("i64,{scenario}"), 1, samples);
            // Date uses i32. Datetime shares the i64 kernel; logical conversion
            // and grouped plugin execution are timed in the Python benchmark.
            if n == 100_000 && ["chain", "dense", "identical", "depth_4"].contains(&scenario) {
                let ds: Vec<_> = s.iter().map(|&s| i32::try_from(s).unwrap()).collect();
                let de: Vec<_> = e.iter().map(|&e| i32::try_from(e).unwrap()).collect();
                measure(
                    &mut file,
                    &ds,
                    &de,
                    &format!("date32,{scenario}"),
                    1,
                    samples,
                );
                for groups in [10, 100, 1000] {
                    measure(
                        &mut file,
                        &s,
                        &e,
                        &format!("i64,{scenario}"),
                        groups,
                        samples,
                    );
                }
            }
            eprintln!("verified and measured {scenario}: {n} rows");
        }
    }
}
