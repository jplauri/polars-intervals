//! Complete A/B/C calls and separate requested-live-heap measurements.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/set_geometry_candidates.rs"]
mod candidates;
#[path = "../tests/support/set_geometry.rs"]
mod oracle;
#[path = "support/random.rs"]
mod random;

use std::{fmt::Debug, hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

// family, left order, right order, left share (0=empty, 1=tiny, 50=balanced, 99=huge).
const CASES: &[(&str, &str, &str, usize)] = &[
    ("availability", "start", "start", 50),
    ("availability", "shuffled", "reverse", 50),
    ("dense", "shuffled", "shuffled", 50),
    ("nested", "start", "shuffled", 50),
    ("duplicates", "shuffled", "reverse", 50),
    ("touching", "reverse", "start", 50),
    ("disjoint", "shuffled", "shuffled", 50),
    ("availability", "partial", "reverse", 99),
    ("holes", "start", "shuffled", 1),
    ("availability", "shuffled", "start", 100),
    ("availability", "start", "shuffled", 0),
    ("sparse", "partial", "reverse", 50),
];

fn fixture(n: usize, case: (&str, &str, &str, usize), seed: u64) -> [Vec<i64>; 4] {
    let (family, lo, ro, share) = case;
    let left_n = match share {
        0 => 0,
        1 => n.min(1),
        50 => n / 2,
        99 => n.saturating_sub(8),
        100 => n,
        _ => unreachable!(),
    };
    let side = |count, right: bool, order| {
        let mut rng = seed.wrapping_add(u64::from(right));
        let mut rows: Vec<_> = (0..count)
            .map(|i| {
                let i = i as i64;
                let n = n as i64;
                let shift = i64::from(right);
                match family {
                    "dense" => (
                        i * 2 + shift,
                        i * 2 + shift + (i * 37 + (seed % 101) as i64) % 101 + 1,
                    ),
                    "nested" => (i, 2 * n - i),
                    "duplicates" => (i % 8, i % 8 + 12),
                    "touching" => (i * 4 + shift, i * 4 + shift + 4),
                    "disjoint" => (i * 10 + shift * 5, i * 10 + shift * 5 + 3),
                    "holes" if !right => (0, n * 10),
                    "sparse" => (
                        i * 10_000_000_000 + shift * 2,
                        i * 10_000_000_000 + shift * 2 + 4,
                    ),
                    _ => (i * 10 + shift * 2, i * 10 + if right { 6 } else { 8 }),
                }
            })
            .collect();
        match order {
            "reverse" => rows.reverse(),
            "shuffled" => random::shuffle(&mut rows, &mut rng),
            "partial" => rows.chunks_mut(64).for_each(|part| part.reverse()),
            _ => {}
        }
        rows.into_iter().unzip::<_, _, Vec<_>, Vec<_>>()
    };
    let (ls, le) = side(left_n, false, lo);
    let (rs, re) = side(n - left_n, true, ro);
    [ls, le, rs, re]
}

fn measure<T: Ord + Copy + Debug>(
    file: &mut impl Write,
    data: &[Vec<T>; 4],
    label: &str,
    samples: usize,
    warmups: usize,
) {
    let [ls, le, rs, re] = data;
    let n = ls.len() + rs.len();
    let useful_left = ls.iter().zip(le).filter(|(s, e)| s < e).count();
    let useful_right = rs.iter().zip(re).filter(|(s, e)| s < e).count();
    let p = intervals_core::merge_intervals(ls, le).unwrap().len();
    let q = intervals_core::merge_intervals(rs, re).unwrap().len();
    let mut endpoints: Vec<_> = data.iter().flatten().copied().collect();
    endpoints.sort_unstable();
    endpoints.dedup();
    let distinct = endpoints.len();
    for intersection in [false, true] {
        let operation = if intersection {
            "intersect"
        } else {
            "subtract"
        };
        let run = |method| {
            if intersection {
                candidates::intersect(method, ls, le, rs, re)
            } else {
                candidates::subtract(method, ls, le, rs, re)
            }
            .unwrap()
        };
        let expected = run("union_scan");
        assert!(expected.iter().all(|(s, e)| s < e));
        assert!(expected.windows(2).all(|w| w[0].1 < w[1].0));
        if n <= 64 {
            assert_eq!(expected, oracle::cells(ls, le, rs, re, intersection));
        }
        let memory: Vec<_> = candidates::METHODS
            .iter()
            .map(|method| {
                let (result, bytes, count) = allocations::measure(|| run(method));
                assert_eq!(result, expected, "{method}");
                (bytes, count)
            })
            .collect();
        for _ in 0..warmups {
            for method in candidates::METHODS {
                drop(black_box(run(method)));
            }
        }
        let iterations = if n <= 64 {
            128
        } else if n <= 1000 {
            4
        } else {
            1
        };
        for sample in 0..samples {
            for offset in 0..candidates::METHODS.len() {
                let index = (sample + offset) % candidates::METHODS.len();
                let method = candidates::METHODS[index];
                let begin = Instant::now();
                for _ in 0..iterations {
                    drop(black_box(run(black_box(method))));
                }
                let batch_ns = begin.elapsed().as_nanos();
                let (peak, allocations) = memory[index];
                writeln!(file, "{label},{operation},{n},{},{},{method},{sample},{iterations},{batch_ns},{},{peak},{allocations},{useful_left},{useful_right},{p},{q},{},{distinct}", ls.len(),rs.len(),batch_ns/iterations,expected.len()).unwrap();
            }
        }
    }
    file.flush().unwrap();
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench");
    for intersection in [false, true] {
        assert_eq!(
            oracle::cells(&[0, 1], &[5, 3], &[2], &[4], intersection),
            oracle::bitmap(&[0, 1], &[5, 3], &[2], &[4], intersection)
        );
    }
    let value = |key, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
    let sizes = value("SET_GEOMETRY_SIZES", "8,1000,10000,100000,1000000");
    let seeds = value("SET_GEOMETRY_SEEDS", "7,41");
    let dtypes = value("SET_GEOMETRY_DTYPES", "i64,u64,i16");
    let cases = value("SET_GEOMETRY_CASES", "");
    let samples = value("SET_GEOMETRY_SAMPLES", "5").parse().unwrap();
    let warmups = value("SET_GEOMETRY_WARMUPS", "2").parse().unwrap();
    let path = value("SET_GEOMETRY_CSV", "target/set-geometry-core.csv");
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    let mut file = std::io::BufWriter::new(output);
    writeln!(file,"case,dtype,family,left_order,right_order,left_share,seed,operation,n,left_n,right_n,method,sample,iterations,batch_ns,ns,peak_bytes,allocations,left_useful,right_useful,p,q,z,distinct_endpoints").unwrap();
    for (case_id, &case) in CASES.iter().enumerate() {
        if !cases.is_empty()
            && !cases
                .split(',')
                .any(|i| i.parse::<usize>().unwrap() == case_id)
        {
            continue;
        }
        for n in sizes.split(',').map(|n| n.parse().unwrap()) {
            for seed in seeds.split(',').map(|n| n.parse().unwrap()) {
                let data = fixture(n, case, seed);
                let label = format!("{},{},{},{},{seed}", case.0, case.1, case.2, case.3);
                for dtype in dtypes.split(',') {
                    match dtype {
                        "i64" => measure(
                            &mut file,
                            &data,
                            &format!("{case_id},i64,{label}"),
                            samples,
                            warmups,
                        ),
                        "u64" => {
                            let converted = data.each_ref().map(|values| {
                                values.iter().map(|&x| x as u64 + (1 << 63)).collect()
                            });
                            measure(
                                &mut file,
                                &converted,
                                &format!("{case_id},u64,{label}"),
                                samples,
                                warmups,
                            );
                        }
                        "i16" if data.iter().flatten().all(|&x| i16::try_from(x).is_ok()) => {
                            let converted = data
                                .each_ref()
                                .map(|values| values.iter().map(|&x| x as i16).collect());
                            measure(
                                &mut file,
                                &converted,
                                &format!("{case_id},i16,{label}"),
                                samples,
                                warmups,
                            );
                        }
                        "i16" => {}
                        _ => panic!("unknown dtype"),
                    }
                }
            }
        }
        eprintln!("case {case_id}: {case:?} complete");
    }
}
