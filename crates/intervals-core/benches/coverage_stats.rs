//! Complete calls, rotated candidates, untimed independent checks and heap runs.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/coverage_stats_candidates.rs"]
mod candidates;
#[allow(dead_code, unused_imports)]
#[path = "../src/geometry.rs"]
mod geometry;
#[path = "../tests/support/coverage_stats.rs"]
mod oracle;
#[path = "support/random.rs"]
mod random;

use intervals_core::{
    CoverageEndpoint, CoverageStats, CoverageStatsError, IntervalError, merge_intervals,
    validate_intervals,
};
use random::{random, shuffle};
use std::{hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

struct Case {
    family: &'static str,
    query_order: &'static str,
    source_order: &'static str,
    ratio: &'static str,
}

const CASES: &[Case] = &[
    Case {
        family: "disjoint",
        query_order: "start",
        source_order: "start",
        ratio: "balanced",
    },
    Case {
        family: "genomic",
        query_order: "start",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "genomic",
        query_order: "start",
        source_order: "start",
        ratio: "balanced",
    },
    Case {
        family: "genomic",
        query_order: "start",
        source_order: "shuffled",
        ratio: "few_queries",
    },
    Case {
        family: "genomic",
        query_order: "start",
        source_order: "shuffled",
        ratio: "many_queries",
    },
    Case {
        family: "genomic",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "genomic",
        query_order: "partial",
        source_order: "reverse",
        ratio: "balanced",
    },
    Case {
        family: "genomic",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "few_queries",
    },
    Case {
        family: "genomic",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "many_queries",
    },
    Case {
        family: "outages",
        query_order: "reverse",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "nested",
        query_order: "start",
        source_order: "start",
        ratio: "balanced",
    },
    Case {
        family: "duplicates",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "touching",
        query_order: "start",
        source_order: "reverse",
        ratio: "balanced",
    },
    Case {
        family: "one_run",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "no_hit",
        query_order: "start",
        source_order: "shuffled",
        ratio: "balanced",
    },
    Case {
        family: "empty_heavy",
        query_order: "shuffled",
        source_order: "partial",
        ratio: "balanced",
    },
    Case {
        family: "sparse",
        query_order: "shuffled",
        source_order: "shuffled",
        ratio: "balanced",
    },
];

fn order(rows: &mut [(i64, i64)], kind: &str, rng: &mut u64) {
    match kind {
        "start" => {}
        "reverse" => rows.reverse(),
        "shuffled" => shuffle(rows, rng),
        "partial" => {
            for chunk in rows.chunks_mut(64) {
                shuffle(chunk, rng);
            }
        }
        _ => unreachable!(),
    }
}

type Rows = Vec<(i64, i64)>;

fn dataset(case: &Case, size: usize, seed: u64) -> (Rows, Rows) {
    let small = if size == 0 { 0 } else { (size / 100).max(1) };
    let (n, m) = match case.ratio {
        "few_queries" => (small, size),
        "many_queries" => (size, small),
        _ => (size, size),
    };
    let span = 10 * m.max(1) as i64;
    let run_count = case
        .family
        .strip_prefix("runs_")
        .map(|x| x.parse::<usize>().unwrap());
    let per_run = run_count.map(|p| m.div_ceil(p).max(1));
    let mut rng = seed.max(1);
    let mut sources: Vec<_> = (0..m)
        .map(|i| {
            if let Some(per_run) = per_run {
                let base = (i / per_run * per_run * 4) as i64;
                let start = base + (i % per_run) as i64;
                return (start, start + per_run as i64);
            }
            let i = i as i64;
            match case.family {
                "genomic" => (i * 5, i * 5 + 30 + (random(&mut rng) % 100) as i64),
                "outages" => (i * 80, i * 80 + 1 + (random(&mut rng) % 160) as i64),
                "nested" => (i, 2 * m as i64 - i),
                "duplicates" => (i / 32 * 10, i / 32 * 10 + 8),
                "touching" => (i * 10, i * 10 + 10),
                "one_run" => (i, i + span),
                "empty_heavy" => (i * 10, i * 10 + if i % 4 == 0 { 8 } else { 0 }),
                "sparse" => ((1i64 << 54) + i * 100_000, (1i64 << 54) + i * 100_000 + 3),
                _ => (i * 10, i * 10 + 4),
            }
        })
        .collect();
    let mut queries: Vec<_> = (0..n)
        .map(|i| {
            let i = i as i64;
            if run_count.is_some() {
                let start = i * (4 * m.max(1)) as i64 / n.max(1) as i64;
                return (start, start + 14);
            }
            let x = i * span / n.max(1) as i64;
            match case.family {
                "genomic" => (x / 2, x / 2 + (span / n.max(1) as i64).max(1) * 2 + 100),
                "outages" => (i * 100, i * 100 + 150),
                "nested" => (i, 2 * n as i64 - i),
                "duplicates" => (i / 32 * 10, i / 32 * 10 + 16),
                "no_hit" => (span + x, span + x + 7),
                "empty_heavy" => (x, x + if i % 3 == 0 { 16 } else { 0 }),
                "sparse" => ((1i64 << 54) + i * 100_000, (1i64 << 54) + i * 100_000 + 5),
                _ => (x, x + 14),
            }
        })
        .collect();
    order(&mut sources, case.source_order, &mut rng);
    order(&mut queries, case.query_order, &mut rng);
    (queries, sources)
}

struct Settings {
    samples: usize,
    warmups: usize,
}

fn measure<T: CoverageEndpoint + std::fmt::Debug>(
    file: &mut impl Write,
    q: &[(T, T)],
    s: &[(T, T)],
    label: &str,
    settings: &Settings,
) {
    let (qs, qe): (Vec<_>, Vec<_>) = q.iter().copied().unzip();
    let (ss, se): (Vec<_>, Vec<_>) = s.iter().copied().unzip();
    let expected = candidates::run("binary", &qs, &qe, &ss, &se).unwrap();
    if q.len().saturating_mul(s.len().saturating_mul(s.len())) <= 2_000_000 {
        assert_eq!(expected, oracle::cells(&qs, &qe, &ss, &se));
    }
    // Large fixtures: structural checks and direct original-row counts at up
    // to eight query positions, plus whole-output candidate equality below.
    for (i, row) in expected.iter().enumerate() {
        assert!(0 <= row.covered_length && row.covered_length <= row.query_length);
        assert!(row.overlap_count <= s.len() as u64);
        assert_eq!(row.covered_fraction.is_none(), q[i].0 == q[i].1);
    }
    for i in (0..q.len()).step_by((q.len() / 8).max(1)) {
        let (a, b) = q[i];
        let count = s
            .iter()
            .filter(|&&(s, e)| a < b && s < e && s < b && a < e)
            .count();
        assert_eq!(expected[i].overlap_count, count as u64);
    }
    let p = merge_intervals(&ss, &se).unwrap().len();
    let nonempty = s.iter().filter(|&&(a, b)| a < b).count();
    let mut unique: Vec<_> = s.iter().copied().filter(|&(a, b)| a < b).collect();
    unique.sort_unstable();
    unique.dedup();
    let duplicates = nonempty - unique.len();
    let mut endpoints: Vec<_> = ss.iter().chain(&se).copied().collect();
    endpoints.sort_unstable();
    endpoints.dedup();
    let mut spans: Vec<_> = expected.iter().map(|r| r.query_length).collect();
    spans.sort_unstable();
    let (span_min, span_median, span_max) = if spans.is_empty() {
        (0, 0, 0)
    } else {
        (spans[0], spans[spans.len() / 2], *spans.last().unwrap())
    };
    let memory: Vec<_> = candidates::METHODS
        .iter()
        .map(|method| {
            let (output, peak, count) =
                allocations::measure(|| candidates::run(method, &qs, &qe, &ss, &se).unwrap());
            assert_eq!(output, expected, "{method}");
            (peak, count)
        })
        .collect();
    for warmup in 0..settings.warmups {
        for offset in 0..candidates::METHODS.len() {
            let method = candidates::METHODS[(offset + warmup) % candidates::METHODS.len()];
            drop(black_box(
                candidates::run(method, &qs, &qe, &ss, &se).unwrap(),
            ));
        }
    }
    let iterations = if q.len() + s.len() <= 64 {
        128
    } else if q.len() + s.len() <= 2000 {
        4
    } else {
        1
    };
    for sample in 0..settings.samples {
        for offset in 0..candidates::METHODS.len() {
            let i = (offset + sample) % candidates::METHODS.len();
            let method = candidates::METHODS[i];
            let clock = Instant::now();
            for _ in 0..iterations {
                drop(black_box(
                    candidates::run(
                        method,
                        black_box(&qs),
                        black_box(&qe),
                        black_box(&ss),
                        black_box(&se),
                    )
                    .unwrap(),
                ));
            }
            let ns = clock.elapsed().as_nanos();
            let (peak, allocations) = memory[i];
            writeln!(file,"{label},{},{},{p},{nonempty},{duplicates},{},{span_min},{span_median},{span_max},{method},{sample},{iterations},{ns},{},{peak},{allocations}",q.len(),s.len(),endpoints.len(),ns/iterations).unwrap();
        }
    }
    file.flush().unwrap();
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench (release mode)");
    let value = |key: &str, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
    let sizes: Vec<usize> = value("STATS_SIZES", "0,1,8,1000,10000,100000,1000000")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let seeds: Vec<u64> = value("STATS_SEEDS", "7,41")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let dtypes = value("STATS_DTYPES", "i64,u64,i16");
    let selected = value("STATS_CASES", "");
    let settings = Settings {
        samples: value("STATS_SAMPLES", "5").parse().unwrap(),
        warmups: value("STATS_WARMUPS", "2").parse().unwrap(),
    };
    let path = value("STATS_CSV", "coverage-stats-core.csv");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    writeln!(file,"family,query_order,source_order,ratio,dtype,seed,n,m,p,nonempty_sources,duplicate_sources,distinct_endpoints,query_span_min,query_span_median,query_span_max,algorithm,sample,iterations,total_ns,ns,peak_requested_bytes,allocation_count").unwrap();
    let crossover_cases: Vec<_> = [
        "runs_1",
        "runs_2",
        "runs_8",
        "runs_32",
        "runs_128",
        "runs_1024",
    ]
    .into_iter()
    .flat_map(|family| {
        ["balanced", "few_queries", "many_queries"]
            .into_iter()
            .map(move |ratio| Case {
                family,
                query_order: "shuffled",
                source_order: "shuffled",
                ratio,
            })
    })
    .collect();
    for case in CASES.iter().chain(&crossover_cases) {
        let case_id = format!(
            "{}:{}:{}:{}",
            case.family, case.query_order, case.source_order, case.ratio
        );
        if !selected.is_empty()
            && !selected.split(',').any(|x| {
                x == case.family
                    || x == case_id
                    || (x == "runs" && case.family.starts_with("runs_"))
            })
        {
            continue;
        }
        for &size in &sizes {
            for &seed in &seeds {
                let (q, s) = dataset(case, size, seed);
                for dtype in dtypes.split(',') {
                    let label = format!(
                        "{},{},{},{},{dtype},{seed}",
                        case.family, case.query_order, case.source_order, case.ratio
                    );
                    match dtype {
                        "i64" => measure(&mut file, &q, &s, &label, &settings),
                        "u64" => {
                            let q: Vec<_> = q
                                .iter()
                                .map(|&(a, b)| {
                                    (u64::try_from(a).unwrap(), u64::try_from(b).unwrap())
                                })
                                .collect();
                            let s: Vec<_> = s
                                .iter()
                                .map(|&(a, b)| {
                                    (u64::try_from(a).unwrap(), u64::try_from(b).unwrap())
                                })
                                .collect();
                            measure(&mut file, &q, &s, &label, &settings);
                        }
                        "i16" => {
                            let convert = |rows: &[(i64, i64)]| {
                                rows.iter()
                                    .map(|&(a, b)| {
                                        Some((i16::try_from(a).ok()?, i16::try_from(b).ok()?))
                                    })
                                    .collect::<Option<Vec<_>>>()
                            };
                            if let (Some(q), Some(s)) = (convert(&q), convert(&s)) {
                                measure(&mut file, &q, &s, &label, &settings);
                            }
                        }
                        _ => panic!("unknown dtype {dtype}"),
                    }
                }
                eprintln!("{case_id}: size={size}, seed={seed} complete");
            }
        }
    }
}
