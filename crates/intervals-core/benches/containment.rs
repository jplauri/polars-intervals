#[path = "support/containment.rs"]
mod support;
use std::hint::black_box;
use std::io::Write;
use std::time::Instant;

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
    writeln!(file, "scenario,n,candidate,sample,total_ms,pairs").unwrap();
    let sizes: Vec<usize> = std::env::var("CONTAINMENT_SIZES")
        .unwrap_or_else(|_| "1000,10000,100000,1000000,3000000".into())
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    for scenario in support::SCENARIOS {
        let (s, e) = support::dataset(scenario, 100);
        assert_eq!(
            intervals_core::containment_counts(&s, &e).unwrap(),
            support::naive(&s, &e)
        );
        for &n in &sizes {
            let (s, e) = support::dataset(scenario, n);
            let expected = intervals_core::containment_counts(&s, &e).unwrap();
            let pairs: u64 = expected.iter().map(|&x| x as u64).sum();
            for sample in 0..5 {
                let tick = Instant::now();
                let counts = black_box(intervals_core::containment_counts(
                    black_box(&s),
                    black_box(&e),
                ));
                let total = tick.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(counts.unwrap(), expected);
                writeln!(
                    file,
                    "{scenario},{n},production,{sample},{total:.6},{pairs}"
                )
                .unwrap();
            }
            file.flush().unwrap();
            eprintln!("{scenario}: {n} rows verified and measured");
        }
    }
}
