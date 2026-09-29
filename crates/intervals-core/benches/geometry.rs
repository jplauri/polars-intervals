//! Complete core calls with rotated candidates and separate requested-heap runs.
//! GEOMETRY_CSV must name a new file. All construction/checking is outside timing.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/geometry_candidates.rs"]
mod candidates;
#[path = "../tests/support/geometry.rs"]
mod oracle;
#[path = "support/random.rs"]
mod random;

use candidates::{CLUSTER_METHODS, GAP_METHODS, MERGE_METHODS};
use std::{fmt::Debug, hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

#[derive(Clone, Copy)]
struct Case {
    family: &'static str,
    order: &'static str,
    domain: &'static str,
}

const CASES: &[Case] = &[
    Case {
        family: "clipped_sorted",
        order: "input",
        domain: "partial",
    },
    Case {
        family: "disjoint",
        order: "start",
        domain: "full",
    },
    Case {
        family: "disjoint",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "touching",
        order: "start",
        domain: "full",
    },
    Case {
        family: "touching",
        order: "reverse",
        domain: "full",
    },
    Case {
        family: "chain",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "nested",
        order: "start",
        domain: "full",
    },
    Case {
        family: "nested",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "spanning",
        order: "start",
        domain: "full",
    },
    Case {
        family: "spanning",
        order: "partial",
        domain: "full",
    },
    Case {
        family: "duplicates",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "ties",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "empties",
        order: "shuffled",
        domain: "full",
    },
    Case {
        family: "empty",
        order: "start",
        domain: "full",
    },
    Case {
        family: "mixed",
        order: "partial",
        domain: "full",
    },
    Case {
        family: "mixed",
        order: "shuffled",
        domain: "partial",
    },
    Case {
        family: "mixed",
        order: "shuffled",
        domain: "outside",
    },
    Case {
        family: "mixed",
        order: "shuffled",
        domain: "empty",
    },
    Case {
        family: "huge",
        order: "shuffled",
        domain: "full",
    },
];

fn dataset(case: Case, n: usize, seed: u64) -> (Vec<i64>, Vec<i64>, (i64, i64)) {
    let mut rng = seed;
    let mut rows: Vec<_> = (0..n)
        .map(|row| {
            let i = row as i64;
            let (s, e) = match case.family {
                "clipped_sorted" if row % 3 == 0 => (-10 * n as i64 - i, -10 * n as i64 - i + 1),
                "clipped_sorted" if row % 3 == 2 => (10 * n as i64 + i, 10 * n as i64 + i + 1),
                "clipped_sorted" => (3 * i, 3 * i + 1),
                "disjoint" => (3 * i, 3 * i + 1),
                "touching" => (i, i + 1),
                "chain" => (3 * i, 3 * i + 4),
                "nested" => (i, 3 * n as i64 - i),
                "spanning" if row == 0 => (0, 3 * n as i64 + 100),
                "spanning" => (3 * i, 3 * i + 1),
                "duplicates" => (3 * (i / 32), 3 * (i / 32) + 2),
                "ties" => (i % 32, i % 32 + 1 + i % 8),
                "empties" => (3 * i, 3 * i + i64::from(row.is_multiple_of(5)) * 4),
                "empty" => (i % 32, i % 32),
                "mixed" => (3 * i, 3 * i + (random::random(&mut rng) % 32) as i64),
                "huge" if row + 1 == n => (i64::MAX - 1, i64::MAX),
                "huge" => (
                    i64::MIN + i * 1_000_000_000_000,
                    i64::MIN + i * 1_000_000_000_000 + 1,
                ),
                _ => unreachable!(),
            };
            (s, e)
        })
        .collect();
    match case.order {
        "input" => {}
        "start" => rows.sort_unstable_by_key(|r| r.0),
        "reverse" => rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0)),
        "shuffled" => random::shuffle(&mut rows, &mut rng),
        "partial" => {
            rows.sort_unstable_by_key(|r| r.0);
            for chunk in rows.chunks_mut(32) {
                chunk.reverse();
            }
        }
        _ => unreachable!(),
    }
    let (s, e): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
    let lo = s.iter().copied().min().unwrap_or(0);
    let hi = e.iter().copied().max().unwrap_or(1);
    let domain = match case.domain {
        "full" => (lo.saturating_sub(1), hi.saturating_add(1)),
        "partial" => {
            let span = i128::from(hi) - i128::from(lo);
            (
                (i128::from(lo) + span / 4) as i64,
                (i128::from(lo) + span * 3 / 4) as i64,
            )
        }
        "outside" => (hi + 10, hi + 20),
        "empty" => (lo, lo),
        _ => unreachable!(),
    };
    (s, e, domain)
}

#[derive(Debug, PartialEq, Eq)]
enum Output<T> {
    Labels(Vec<u32>),
    Ranges(Vec<(T, T)>),
}

impl<T> Output<T> {
    fn len(&self) -> usize {
        match self {
            Self::Labels(v) => v.len(),
            Self::Ranges(v) => v.len(),
        }
    }
}

fn run<T: Ord + Copy>(
    method: &str,
    operation: &str,
    s: &[T],
    e: &[T],
    domain: (T, T),
    touching: bool,
) -> Output<T> {
    match operation {
        "cluster" => Output::Labels(candidates::cluster(method, s, e, touching).unwrap()),
        "merge" => Output::Ranges(candidates::merge(method, s, e).unwrap()),
        "gaps" => Output::Ranges(candidates::gaps(method, s, e, domain.0, domain.1).unwrap()),
        _ => unreachable!(),
    }
}

fn verify<T: Ord + Copy + Debug>(
    output: &Output<T>,
    operation: &str,
    s: &[T],
    e: &[T],
    domain: (T, T),
    touching: bool,
) {
    match output {
        Output::Labels(labels) => {
            assert_eq!(labels.len(), s.len());
            let mut counts =
                vec![0usize; labels.iter().copied().max().map_or(0, |v| v as usize + 1)];
            let mut seen = 0;
            for &label in labels {
                if counts[label as usize] == 0 {
                    assert_eq!(label as usize, seen);
                    seen += 1;
                }
                counts[label as usize] += 1;
            }
            for i in 0..s.len() {
                if s[i] == e[i] {
                    assert_eq!(counts[labels[i] as usize], 1);
                }
            }
            if s.len() <= 64 {
                assert_eq!(*labels, oracle::graph(s, e, touching));
            }
        }
        Output::Ranges(ranges) => {
            assert!(ranges.iter().all(|(a, b)| a < b));
            assert!(ranges.windows(2).all(|w| w[0].1 < w[1].0));
            if operation == "gaps" {
                assert!(ranges.iter().all(|&(a, b)| domain.0 <= a && b <= domain.1));
            }
            if s.len() <= 64 {
                assert_eq!(
                    *ranges,
                    oracle::cells(s, e, (operation == "gaps").then_some(domain))
                );
            }
        }
    }
}

struct Settings {
    samples: usize,
    warmups: usize,
    operations: String,
}

fn measure<T: Ord + Copy + Debug>(
    file: &mut impl Write,
    s: &[T],
    e: &[T],
    domain: (T, T),
    label: &str,
    settings: &Settings,
) {
    let nonempty = s.iter().zip(e).filter(|(a, b)| a < b).count();
    let clipped = s
        .iter()
        .zip(e)
        .filter(|&(&a, &b)| a.max(domain.0) < b.min(domain.1))
        .count();
    let runs = intervals_core::merge_intervals(s, e).unwrap().len();
    for operation in settings.operations.split(',') {
        let methods = match operation {
            "cluster" => CLUSTER_METHODS,
            "merge" => MERGE_METHODS,
            "gaps" => GAP_METHODS,
            _ => panic!("unknown operation"),
        };
        for touching in [false, true] {
            if operation != "cluster" && touching {
                continue;
            }
            let expected = run("production", operation, s, e, domain, touching);
            verify(&expected, operation, s, e, domain, touching);
            let components = if let Output::Labels(ids) = &expected {
                ids.iter().copied().max().map_or(0, |id| id as usize + 1)
            } else {
                0
            };
            let memory: Vec<_> = methods
                .iter()
                .map(|method| {
                    let (output, peak, count) =
                        allocations::measure(|| run(method, operation, s, e, domain, touching));
                    assert_eq!(output, expected, "{method}");
                    (peak, count)
                })
                .collect();
            for warmup in 0..settings.warmups {
                for offset in 0..methods.len() {
                    drop(black_box(run(
                        methods[(warmup + offset) % methods.len()],
                        operation,
                        s,
                        e,
                        domain,
                        touching,
                    )));
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
                    let start = Instant::now();
                    for _ in 0..iterations {
                        drop(black_box(run(
                            method,
                            operation,
                            black_box(s),
                            black_box(e),
                            domain,
                            touching,
                        )));
                    }
                    let ns = start.elapsed().as_nanos();
                    let (peak, allocations) = memory[i];
                    writeln!(file,"{label},{operation},{touching},{},{method},{sample},{iterations},{ns},{},{peak},{allocations},{nonempty},{clipped},{components},{runs},{},{:?},{:?},1",s.len(),ns/iterations,expected.len(),domain.0,domain.1).unwrap();
                }
            }
        }
    }
    file.flush().unwrap();
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench (release mode)");
    let value = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let sizes: Vec<usize> = value("GEOMETRY_SIZES", "0,1,8,1000,10000,100000,1000000")
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    let seeds: Vec<u64> = value("GEOMETRY_SEEDS", "7,41")
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    let settings = Settings {
        samples: value("GEOMETRY_SAMPLES", "5").parse().unwrap(),
        warmups: value("GEOMETRY_WARMUPS", "2").parse().unwrap(),
        operations: value("GEOMETRY_OPERATIONS", "cluster,merge,gaps"),
    };
    assert!(settings.samples > 0 && settings.warmups > 0);
    let filter = value("GEOMETRY_CASES", "");
    let dtypes = value("GEOMETRY_DTYPES", "i64,u64,i16");
    let path = value(
        "GEOMETRY_CSV",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/geometry-core.csv"
        ),
    );
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("GEOMETRY_CSV must name a new file");
    let mut file = std::io::BufWriter::new(output);
    writeln!(file,"dtype,family,order,domain,seed,operation,include_touching,n,method,sample,iterations,batch_ns,total_ns,peak_bytes,allocations,nonempty,clipped,components,runs,z,domain_start,domain_end,group_count").unwrap();
    eprintln!(
        "scope=complete core call including validation,preparation,clipping,sorting,restoration,output/destruction; warmups={}; samples={}; memory=separate untimed peak requested heap including output, excluding inputs, allocator overhead, stack and RSS; graph/cell oracle n<=64, candidate agreement and invariants at larger n; i16 omits coordinates outside its range",
        settings.warmups, settings.samples
    );
    // The bounded bitmap is an independent second oracle for a tiny fixture.
    assert_eq!(
        oracle::cells(&[1, 3], &[2, 5], Some((0, 6))),
        oracle::bitmap(&[1, 3], &[2, 5], Some((0, 6)))
    );
    for &case in CASES {
        let name = format!("{}-{}-{}", case.family, case.order, case.domain);
        if !filter.is_empty() && !filter.split(',').any(|x| x == name) {
            continue;
        }
        for &n in &sizes {
            for &seed in &seeds {
                let (s, e, domain) = dataset(case, n, seed);
                let label = format!("{},{},{},{seed}", case.family, case.order, case.domain);
                for dtype in dtypes.split(',') {
                    match dtype {
                        "i64" => measure(
                            &mut file,
                            &s,
                            &e,
                            domain,
                            &format!("i64,{label}"),
                            &settings,
                        ),
                        "u64" => {
                            let map = |x: i64| (i128::from(x) - i128::from(i64::MIN)) as u64;
                            let ss: Vec<_> = s.iter().copied().map(map).collect();
                            let ee: Vec<_> = e.iter().copied().map(map).collect();
                            measure(
                                &mut file,
                                &ss,
                                &ee,
                                (map(domain.0), map(domain.1)),
                                &format!("u64,{label}"),
                                &settings,
                            );
                        }
                        "i16"
                            if s.iter()
                                .chain(&e)
                                .chain([&domain.0, &domain.1])
                                .all(|&x| i16::try_from(x).is_ok()) =>
                        {
                            let ss: Vec<_> = s.iter().map(|&x| x as i16).collect();
                            let ee: Vec<_> = e.iter().map(|&x| x as i16).collect();
                            measure(
                                &mut file,
                                &ss,
                                &ee,
                                (domain.0 as i16, domain.1 as i16),
                                &format!("i16,{label}"),
                                &settings,
                            );
                        }
                        "i16" => {}
                        _ => panic!("unknown dtype {dtype}"),
                    }
                }
            }
        }
        eprintln!("finished {name}");
    }
}
