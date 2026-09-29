//! Complete calls, rotated methods, separate untimed requested-heap measurement.
//! PROFILE_CSV must name a NEW file; PROFILE_* settings are read in main.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/coverage_profile_candidates.rs"]
mod candidates;
#[path = "support/random.rs"]
mod random;

use candidates::{METHODS, oracle, run};
use intervals_core::CoverageSegment;
use random::{random, shuffle};
use std::{
    hint::black_box,
    io::Write,
    mem::{align_of, size_of},
    time::Instant,
};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

#[derive(Clone, Copy)]
struct Case {
    family: &'static str,
    order: &'static str,
    weights: &'static str,
    domain: &'static str,
    zero: bool,
    million: bool,
}

macro_rules! cases {
    ($(($f:literal, $o:literal, $w:literal, $d:literal, $z:literal, $m:literal)),* $(,)?) => {
        &[$(Case { family: $f, order: $o, weights: $w, domain: $d, zero: $z, million: $m }),*]
    };
}

const CASES: &[Case] = cases![
    ("disjoint", "start", "units", "inferred", false, true),
    (
        "disjoint",
        "shuffled",
        "heterogeneous",
        "extended",
        true,
        true
    ),
    ("touching", "start", "units", "inferred", false, true),
    ("touching", "reverse", "uniform", "inferred", true, false),
    ("sequencing", "start", "units", "inferred", false, true),
    ("sequencing", "shuffled", "units", "inferred", false, true),
    ("sequencing", "start", "ones", "inferred", false, false),
    ("booking", "start", "heterogeneous", "inferred", false, true),
    ("booking", "end", "heterogeneous", "partial", true, false),
    ("booking", "partial", "mixed", "extended", true, false),
    ("booking", "shuffled", "skewed", "inferred", false, true),
    ("moderate", "shuffled", "units", "inferred", false, true),
    (
        "clique",
        "shuffled",
        "heterogeneous",
        "inferred",
        false,
        true
    ),
    ("nesting", "start", "units", "inferred", false, true),
    ("nesting", "end", "heterogeneous", "inferred", false, false),
    ("duplicates", "shuffled", "units", "inferred", false, true),
    (
        "repeated",
        "shuffled",
        "heterogeneous",
        "inferred",
        false,
        true
    ),
    ("net_zero", "shuffled", "uniform", "inferred", false, true),
    ("gaps", "partial", "mixed", "extended", true, false),
    ("many_empty", "shuffled", "mixed", "inferred", true, false),
    ("empty", "start", "units", "inferred", false, true),
    (
        "empty",
        "shuffled",
        "heterogeneous",
        "extended",
        true,
        false
    ),
    ("zero", "shuffled", "zero", "inferred", true, true),
    (
        "booking",
        "shuffled",
        "heterogeneous",
        "outside",
        false,
        true
    ),
    ("booking", "shuffled", "heterogeneous", "empty", true, false),
    ("sparse", "shuffled", "units", "inferred", true, true),
];

fn dataset(case: Case, n: usize, seed: u64) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let mut rng = seed;
    let mut rows: Vec<_> = (0..n)
        .map(|index| {
            let i = index as i64;
            let size = n as i64;
            let (s, e) = match case.family {
                "disjoint" => (3 * i, 3 * i + 1),
                "touching" => (i, i + 1),
                "sequencing" => (3 * i, 3 * i + 16),
                "booking" => (3 * i, 3 * i + 1 + (random(&mut rng) % 128) as i64),
                "moderate" => (i, i + 64),
                "clique" => (i, size + i + 1),
                "nesting" => (i, 2 * size - i),
                "duplicates" => (3 * (i / 32), 3 * (i / 32) + 2),
                "repeated" => (i % 32, i % 32 + 16),
                "net_zero" => (i / 16, i / 16 + 1),
                "gaps" => (1000 * (i / 16) + i % 16, 1000 * (i / 16) + i % 16 + 8),
                "many_empty" => (i, i + i64::from(i % 5 == 0) * 32),
                "empty" => (i, i),
                "zero" => (i, i + 10),
                "sparse" if index + 1 == n => (i64::MAX - 1, i64::MAX),
                "sparse" => (
                    i64::MIN + i * 1_000_000_000_000,
                    i64::MIN + i * 1_000_000_000_000 + 1,
                ),
                _ => unreachable!(),
            };
            let w = match case.weights {
                "units" | "ones" => 1,
                "uniform" => 7,
                "heterogeneous" => 1 + (random(&mut rng) % 17) as i64,
                "mixed" => (random(&mut rng) % 4) as i64,
                "skewed" if random(&mut rng).is_multiple_of(100) => 1_000_000,
                "skewed" => 1,
                "zero" => 0,
                _ => unreachable!(),
            };
            (s, e, w)
        })
        .collect();
    match case.order {
        "start" => rows.sort_unstable_by_key(|r| r.0),
        "end" => rows.sort_unstable_by_key(|r| r.1),
        "reverse" => rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0)),
        "shuffled" => shuffle(&mut rows, &mut rng),
        "partial" => {
            rows.sort_unstable_by_key(|r| r.0);
            for chunk in rows.chunks_mut(32) {
                chunk.reverse();
            }
        }
        _ => unreachable!(),
    }
    let (mut s, mut e, mut w) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for (start, end, weight) in rows {
        s.push(start);
        e.push(end);
        w.push(weight);
    }
    (s, e, w)
}

fn domain(case: Case, s: &[i64], e: &[i64]) -> Option<(i64, i64)> {
    let (lo, hi) = (
        s.iter().copied().min().unwrap_or(0),
        e.iter().copied().max().unwrap_or(0),
    );
    match case.domain {
        "inferred" => None,
        "extended" => Some((lo - 10, hi + 10)),
        "partial" => Some((lo + (hi - lo) / 4, lo + (hi - lo) * 3 / 4)),
        "outside" => Some((hi + 10, hi + 20)),
        "empty" => Some((lo, lo)),
        _ => unreachable!(),
    }
}

fn stats<T: Ord + Copy, W: Copy>(
    s: &[T],
    e: &[T],
    w: Option<&[W]>,
    domain: Option<(T, T)>,
) -> (usize, usize, usize)
where
    i128: From<W>,
{
    let domain = domain.or_else(|| {
        let lo = (0..s.len()).filter(|&i| s[i] < e[i]).map(|i| s[i]).min()?;
        let hi = (0..s.len()).filter(|&i| s[i] < e[i]).map(|i| e[i]).max()?;
        Some((lo, hi))
    });
    let Some((lo, hi)) = domain.filter(|(a, b)| a < b) else {
        return (0, 0, 0);
    };
    let mut events = Vec::new();
    let mut coordinates = vec![lo, hi];
    for i in 0..s.len() {
        let (start, end) = (s[i].max(lo), e[i].min(hi));
        if start < end && w.is_none_or(|w| i128::from(w[i]) > 0) {
            events.push((start, 1i32));
            events.push((end, -1i32));
            coordinates.push(start);
            coordinates.push(end);
        }
    }
    let m = events.len() / 2;
    events.sort_unstable();
    let (mut active, mut omega) = (0i32, 0i32);
    for (_, delta) in events {
        active += delta;
        omega = omega.max(active);
    }
    coordinates.sort_unstable();
    coordinates.dedup();
    (m, coordinates.len(), omega as usize)
}

fn verify<T: Ord + Copy + std::fmt::Debug>(
    s: &[T],
    e: &[T],
    domain: Option<(T, T)>,
    zero: bool,
    output: &[CoverageSegment<T>],
) {
    assert!(output.len() <= 2 * s.len() + 1);
    for cell in output {
        assert!(cell.start < cell.end);
        assert!(cell.load >= i128::from(!zero));
        if let Some((lo, hi)) = domain {
            assert!(lo <= cell.start && cell.end <= hi);
        }
    }
    for pair in output.windows(2) {
        assert!(pair[0].end <= pair[1].start);
        assert!(pair[0].end != pair[1].start || pair[0].load != pair[1].load);
    }
    // Sorted boundary membership is O(n log n + z log n), not a quadratic oracle.
    let mut points = s.iter().chain(e).copied().collect::<Vec<_>>();
    if let Some((a, b)) = domain {
        points.extend([a, b]);
    }
    points.sort_unstable();
    for segment in output {
        assert!(points.binary_search(&segment.start).is_ok());
        assert!(points.binary_search(&segment.end).is_ok());
    }
}

struct Settings {
    samples: usize,
    warmups: usize,
}

fn measure<T: Ord + Copy + std::fmt::Debug, W: Copy>(
    file: &mut impl Write,
    data: (&[T], &[T], Option<&[W]>),
    domain: Option<(T, T)>,
    case: Case,
    label: &str,
    settings: &Settings,
) where
    i128: From<W>,
{
    let (s, e, w) = data;
    let methods: Vec<_> = METHODS
        .iter()
        .copied()
        .filter(|&method| w.is_some() || method != "streams_indices")
        .collect();
    let expected = run("production", s, e, w, domain, case.zero).unwrap();
    verify(s, e, domain, case.zero, &expected);
    if s.len() <= 1000 {
        assert_eq!(expected, oracle(s, e, w, domain, case.zero).unwrap());
    }
    let (m, u, omega) = stats(s, e, w, domain);
    let memory: Vec<_> = methods
        .iter()
        .map(|method| {
            let (output, peak, count) =
                allocations::measure(|| run(method, s, e, w, domain, case.zero).unwrap());
            assert_eq!(output, expected, "{method}");
            (peak, count)
        })
        .collect();
    for warmup in 0..settings.warmups {
        for offset in 0..methods.len() {
            drop(black_box(
                run(
                    methods[(offset + warmup) % methods.len()],
                    s,
                    e,
                    w,
                    domain,
                    case.zero,
                )
                .unwrap(),
            ));
        }
    }
    let iterations = if s.len() <= 64 {
        128
    } else if s.len() <= 1000 {
        4
    } else {
        1
    };
    for sample in 0..settings.samples {
        for offset in 0..methods.len() {
            let i = (sample + offset) % methods.len();
            let method = methods[i];
            let tick = Instant::now();
            for _ in 0..iterations {
                drop(black_box(
                    run(
                        method,
                        black_box(s),
                        black_box(e),
                        black_box(w),
                        domain,
                        case.zero,
                    )
                    .unwrap(),
                ));
            }
            let ns = tick.elapsed().as_nanos();
            let (peak, count) = memory[i];
            writeln!(file, "{label},{},{method},{sample},{iterations},{ns},{},{peak},{count},{m},{u},{},{omega},{},{},{},{},{},{}", s.len(), ns / iterations, expected.len(), size_of::<(T,usize)>(), align_of::<(T,usize)>(), size_of::<CoverageSegment<T>>(), align_of::<CoverageSegment<T>>(), w.map_or(0, |_| size_of::<(T,W)>()), w.map_or(0, |_| align_of::<(T,W)>())).unwrap();
        }
    }
    file.flush().unwrap();
}

fn measure_weights<T: Ord + Copy + std::fmt::Debug>(
    file: &mut impl Write,
    data: (&[T], &[T], Option<&[i64]>),
    domain: Option<(T, T)>,
    case: Case,
    label: &str,
    settings: &Settings,
    weight_dtypes: &str,
) {
    let (s, e, w) = data;
    if w.is_none() || weight_dtypes.split(',').any(|dtype| dtype == "i64") {
        let dtype = if w.is_none() { "unit" } else { "i64" };
        measure(
            file,
            data,
            domain,
            case,
            &format!("{label},{dtype}"),
            settings,
        );
    }
    if let Some(w) = w
        && weight_dtypes.split(',').any(|dtype| dtype == "i128")
    {
        let wide: Vec<_> = w.iter().map(|&value| i128::from(value)).collect();
        measure(
            file,
            (s, e, Some(wide.as_slice())),
            domain,
            case,
            &format!("{label},i128"),
            settings,
        );
    }
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench (release mode)");
    let value = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let sizes: Vec<usize> = value("PROFILE_SIZES", "0,1,8,1000,10000,100000,1000000")
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    let seeds: Vec<u64> = value("PROFILE_SEEDS", "7,41")
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    let settings = Settings {
        samples: value("PROFILE_SAMPLES", "5").parse().unwrap(),
        warmups: value("PROFILE_WARMUPS", "2").parse().unwrap(),
    };
    assert!(settings.samples > 0 && settings.warmups > 0);
    let filter = value("PROFILE_CASES", "");
    let dtypes = value("PROFILE_DTYPES", "i64,u64");
    let weight_dtypes = value("PROFILE_WEIGHT_DTYPES", "i64");
    let path = value(
        "PROFILE_CSV",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/coverage-profile.csv"
        ),
    );
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("PROFILE_CSV must name a new file");
    let mut file = std::io::BufWriter::new(output);
    writeln!(file, "dtype,family,order,weights,domain,include_zero,seed,weight_dtype,n,method,sample,iterations,batch_ns,total_ns,peak_bytes,allocations,m,u,z,omega,index_record_bytes,index_record_align,segment_bytes,segment_align,weight_record_bytes,weight_record_align").unwrap();
    eprintln!(
        "scope=complete core call including validation/clipping/sortedness/preparation/coalescing/output/destruction; warmups={}; samples={}; memory=separate untimed peak requested heap excluding inputs; small independent oracle n<=1000; large candidate agreement plus invariants",
        settings.warmups, settings.samples
    );
    for &case in CASES {
        let name = format!(
            "{}-{}-{}-{}-{}",
            case.family, case.order, case.weights, case.domain, case.zero
        );
        if !filter.is_empty() && !filter.split(',').any(|x| x == name) {
            continue;
        }
        for &n in &sizes {
            if n >= 1_000_000 && !case.million {
                continue;
            }
            for &seed in &seeds {
                let (s, e, weights) = dataset(case, n, seed);
                let w = (case.weights != "units").then_some(weights.as_slice());
                let bounds = domain(case, &s, &e);
                let label = format!(
                    "{},{},{},{},{},{seed}",
                    case.family, case.order, case.weights, case.domain, case.zero
                );
                if dtypes.split(',').any(|x| x == "i64") {
                    measure_weights(
                        &mut file,
                        (&s, &e, w),
                        bounds,
                        case,
                        &format!("i64,{label}"),
                        &settings,
                        &weight_dtypes,
                    );
                }
                if dtypes.split(',').any(|x| x == "u64") {
                    let unsigned = |x: i64| (i128::from(x) - i128::from(i64::MIN)) as u64;
                    let us: Vec<_> = s.iter().copied().map(unsigned).collect();
                    let ue: Vec<_> = e.iter().copied().map(unsigned).collect();
                    let ub = bounds.map(|(a, b)| (unsigned(a), unsigned(b)));
                    measure_weights(
                        &mut file,
                        (&us, &ue, w),
                        ub,
                        case,
                        &format!("u64,{label}"),
                        &settings,
                        &weight_dtypes,
                    );
                }
            }
            eprintln!("verified and measured {name}: {n}");
        }
    }
}
