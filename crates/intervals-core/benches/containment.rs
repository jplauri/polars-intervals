#[path = "support/containment.rs"]
mod support;
use std::hint::black_box;
use std::io::Write;
use std::time::Instant;
use support::{Fenwick, Segment, indirect, packed};

fn main() {
    if cfg!(debug_assertions) {
        panic!("run with cargo bench, not debug mode");
    }
    let path = std::env::var("CONTAINMENT_CSV").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/containment-kernels.csv"
        )
        .into()
    });
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(file, "scenario,n,candidate,sample,total_ms,compression_ms,sort_ms,sweep_ms,vec_capacity_bytes,pairs").unwrap();
    let sizes: Vec<usize> = std::env::var("CONTAINMENT_SIZES")
        .unwrap_or_else(|_| "1000,10000,100000,1000000,3000000".into())
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let candidates = [
        (
            "packed_fenwick",
            packed::<i64, Fenwick> as fn(&[i64], &[i64]) -> _,
        ),
        ("indirect_fenwick", indirect::<i64>),
        ("packed_segment", packed::<i64, Segment>),
    ];
    for scenario in support::SCENARIOS {
        let (s, e) = support::dataset(scenario, 100);
        let oracle = support::naive(&s, &e);
        for (_, run) in candidates {
            assert_eq!(run(&s, &e).unwrap().counts, oracle);
        }
        for &n in &sizes {
            let (s, e) = support::dataset(scenario, n);
            let expected = intervals_core::containment_counts(&s, &e).unwrap();
            // All full-size outputs agree before accepting any timings.
            for (_, run) in candidates {
                assert_eq!(run(&s, &e).unwrap().counts, expected);
            }
            let pairs: u64 = expected.iter().map(|&x| x as u64).sum();
            for sample in 0..5 {
                for offset in 0..candidates.len() {
                    let (name, run) = candidates[(sample + offset) % candidates.len()];
                    let tick = Instant::now();
                    let result = black_box(run(black_box(&s), black_box(&e)).unwrap());
                    let total = tick.elapsed().as_secs_f64() * 1000.0;
                    assert_eq!(result.counts, expected);
                    writeln!(
                        file,
                        "{scenario},{n},{name},{sample},{total:.6},{:.6},{:.6},{:.6},{},{pairs}",
                        result.compression.as_secs_f64() * 1000.0,
                        result.sorting.as_secs_f64() * 1000.0,
                        result.sweep.as_secs_f64() * 1000.0,
                        result.bytes
                    )
                    .unwrap();
                }
            }
            file.flush().unwrap();
            eprintln!("{scenario}: {n} rows verified and measured");
        }
    }
}
