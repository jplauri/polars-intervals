//! Release-only candidate experiment. Environment: NESTING_CSV, NESTING_SIZES,
//! NESTING_SAMPLES, NESTING_SCENARIOS, NESTING_METHODS. Defaults run the complete matrix.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/nesting_depth.rs"]
mod candidates;

use std::{hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn grouped<T: Ord + Copy>(
    s: &[T],
    e: &[T],
    method: &str,
    groups: usize,
) -> candidates::Measurement {
    let run = |s: &[T], e: &[T]| {
        if method == "production" {
            candidates::Measurement {
                depths: intervals_core::nesting_depths(s, e).unwrap(),
                ..Default::default()
            }
        } else {
            candidates::run(s, e, method).unwrap()
        }
    };
    if groups == 1 {
        return run(s, e);
    }
    let mut result = candidates::Measurement::default();
    let tick = Instant::now();
    result.depths = Vec::with_capacity(s.len());
    result.output_init = tick.elapsed();
    for group in 0..groups {
        let start = group * s.len() / groups;
        let end = (group + 1) * s.len() / groups;
        let current = run(&s[start..end], &e[start..end]);
        result.validation += current.validation;
        result.preparation += current.preparation;
        result.sorting += current.sorting;
        result.compression += current.compression;
        result.output_init += current.output_init;
        result.dp_and_scatter += current.dp_and_scatter;
        let tick = Instant::now();
        result.depths.extend(current.depths);
        result.output_init += tick.elapsed();
    }
    result
}

fn measure<T: Ord + Copy>(
    file: &mut impl Write,
    s: &[T],
    e: &[T],
    case: &str,
    groups: usize,
    samples: usize,
    methods: &[&str],
) {
    // Verify every full-size candidate before collecting any accepted timing.
    let expected = grouped(s, e, candidates::METHODS[0], groups).depths;
    for group in 0..groups {
        let start = group * s.len() / groups;
        let end = (group + 1) * s.len() / groups;
        assert_eq!(
            intervals_core::nesting_depths(&s[start..end], &e[start..end]).unwrap(),
            expected[start..end],
            "production/{case}"
        );
    }
    let methods: Vec<_> = methods
        .iter()
        .map(|&method| {
            let (result, peak, count) = allocations::measure(|| grouped(s, e, method, groups));
            assert_eq!(result.depths, expected, "{case}/{method}");
            (method, peak, count)
        })
        .collect();
    let max_depth = expected.iter().copied().max().unwrap_or(0);
    for sample in 0..samples {
        // Rotate candidates to avoid always giving the same one the warm cache.
        for offset in 0..methods.len() {
            let (method, peak, count) = methods[(sample + offset) % methods.len()];
            let tick = Instant::now();
            let result = black_box(grouped(black_box(s), black_box(e), method, groups));
            let total = tick.elapsed().as_nanos();
            assert_eq!(result.depths, expected, "{case}/{method}");
            let phases = if method == "production" {
                // The production call has no phase clocks; blanks mean unmeasured.
                ",,,,,".to_owned()
            } else {
                format!(
                    "{},{},{},{},{},{}",
                    result.validation.as_nanos(),
                    result.preparation.as_nanos(),
                    result.sorting.as_nanos(),
                    result.compression.as_nanos(),
                    result.output_init.as_nanos(),
                    result.dp_and_scatter.as_nanos()
                )
            };
            writeln!(
                file,
                "{case},{},{groups},{method},{sample},{total},{phases},{peak},{count},{max_depth}",
                s.len()
            )
            .unwrap();
        }
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
        std::env::var("NESTING_SCENARIOS").unwrap_or_else(|_| candidates::SCENARIOS.join(","));
    let methods = std::env::var("NESTING_METHODS")
        .unwrap_or_else(|_| format!("{},production", candidates::METHODS.join(",")));
    let methods: Vec<_> = methods.split(',').collect();
    let path = std::env::var("NESTING_CSV").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/nesting-depth.csv"
        )
        .into()
    });
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(file, "dtype,scenario,n,groups,method,sample,total_ns,validation_ns,preparation_ns,sorting_ns,compression_ns,output_init_ns,dp_and_scatter_ns,peak_bytes,allocations,max_depth").unwrap();
    for scenario in scenarios.split(',') {
        let (s, e) = candidates::dataset(scenario, 100);
        let oracle = candidates::naive(&s, &e);
        for method in candidates::METHODS {
            assert_eq!(
                candidates::run(&s, &e, method).unwrap().depths,
                oracle,
                "{scenario}/{method}"
            );
        }
        for &n in &sizes {
            let (s, e) = candidates::dataset(scenario, n);
            measure(
                &mut file,
                &s,
                &e,
                &format!("i64,{scenario}"),
                1,
                samples,
                &methods,
            );
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
                    &methods,
                );
            }
            if n == 100_000 && ["chain", "dense", "identical", "depth_4"].contains(&scenario) {
                for groups in [10, 100, 1000] {
                    measure(
                        &mut file,
                        &s,
                        &e,
                        &format!("i64,{scenario}"),
                        groups,
                        samples,
                        &methods,
                    );
                }
            }
            eprintln!("verified and measured {scenario}: {n} rows");
        }
    }
}
