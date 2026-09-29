//! Complete-call experiments; DOMINATION_* variables select new output and cases.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/domination.rs"]
mod candidates;
#[path = "support/random.rs"]
mod random;

use intervals_core::IntervalError;
use random::{random, shuffle};
use std::{hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

// Focused matrix: geometry, order, costs, eligible for million-row runs.
const CASES: &[(&str, &str, &str, bool)] = &[
    ("disjoint", "shuffled", "positive", true),
    ("touching", "reverse", "units", true),
    ("path", "start", "units", true),
    ("path", "shuffled", "ones", false),
    ("path", "shuffled", "zero", false),
    ("path", "shuffled", "mixed", true),
    ("clique", "shuffled", "positive", true),
    ("nested", "end", "positive", true),
    ("nested", "start", "units", true),
    ("star", "shuffled", "cheap_hub", false),
    ("star", "reverse", "expensive_hub", true),
    ("layered", "partial", "skewed", true),
    ("duplicate", "shuffled", "mixed", true),
    ("duplicate", "shuffled", "skewed", false),
    ("duplicate", "reverse", "units", false),
    ("proper", "end", "positive", true),
    ("components", "shuffled", "mixed", false),
    ("empty", "start", "units", true),
    ("empty", "shuffled", "mixed", false),
    ("many_empty", "partial", "positive", true),
    ("sparse", "shuffled", "units", true),
];

fn dataset(
    family: &str,
    order: &str,
    mode: &str,
    n: usize,
    seed: u64,
) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let mut rng = seed;
    let mut rows: Vec<_> = (0..n)
        .map(|i| {
            let x = i as i64;
            let (s, e) = match family {
                "disjoint" => (3 * x, 3 * x + 1),
                "touching" => (x, x + 1),
                "path" => (3 * x, 3 * x + 4),
                "clique" => (x, n as i64 + x + 1),
                "nested" => (x, 2 * n as i64 - x + 1),
                "star" if i == 0 => (0, 3 * n as i64 + 1),
                "star" => (3 * x, 3 * x + 1),
                "layered" => (x, x + 1 + (random(&mut rng) % 128) as i64),
                "duplicate" => (x / 32 * 3, x / 32 * 3 + 2),
                "proper" => (x, x + 16),
                "components" => (x / 32 * 1000 + x % 32, x / 32 * 1000 + x % 32 + 8),
                "empty" => (x % 16, x % 16),
                "many_empty" => (x, x + if i % 5 == 0 { 32 } else { 0 }),
                "sparse" if i + 1 == n => (i64::MAX - 1, i64::MAX),
                "sparse" => (
                    i64::MIN + x * 1_000_000_000_000,
                    i64::MIN + x * 1_000_000_000_000 + 1,
                ),
                _ => unreachable!(),
            };
            let cost = match mode {
                "units" | "ones" => 1,
                "zero" => 0,
                "positive" => (random(&mut rng) % 23 + 1) as i64,
                "mixed" => (random(&mut rng) % 7) as i64,
                "skewed" => {
                    if random(&mut rng).is_multiple_of(10) {
                        1_000_000
                    } else {
                        1
                    }
                }
                "cheap_hub" => {
                    if i == 0 {
                        1
                    } else {
                        100
                    }
                }
                "expensive_hub" => {
                    if i == 0 {
                        n as i64 + 1
                    } else {
                        1
                    }
                }
                _ => unreachable!(),
            };
            (s, e, cost)
        })
        .collect();
    match order {
        "start" => rows.sort_unstable_by_key(|r| r.0),
        "end" => rows.sort_unstable_by_key(|r| r.1),
        "reverse" => rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0)),
        "partial" => {
            rows.sort_unstable_by_key(|r| r.0);
            for chunk in rows.chunks_mut(32) {
                chunk.reverse();
            }
        }
        "shuffled" => shuffle(&mut rows, &mut rng),
        _ => unreachable!(),
    }
    let (mut s, mut e, mut c) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for row in rows {
        s.push(row.0);
        e.push(row.1);
        c.push(row.2);
    }
    (s, e, c)
}

fn run<T: Ord + Copy>(s: &[T], e: &[T], w: Option<&[i64]>, method: &str) -> Vec<bool> {
    if method == "production" {
        w.map_or_else(
            || intervals_core::minimum_dominating_set(s, e),
            |w| intervals_core::minimum_cost_dominating_set(s, e, w),
        )
        .unwrap()
    } else {
        candidates::run(s, e, w, method).unwrap()
    }
}

// Independent original-graph feasibility, O(n log n), with no reduced blocks.
fn verify<T: Ord + Copy>(s: &[T], e: &[T], w: Option<&[i64]>, mask: &[bool]) -> (i128, usize) {
    assert_eq!(s.len(), mask.len());
    let mut selected: Vec<_> = (0..s.len())
        .filter(|&i| mask[i] && s[i] < e[i])
        .map(|i| (s[i], e[i]))
        .collect();
    selected.sort_unstable();
    let mut prefix = Vec::with_capacity(selected.len());
    for &(_, end) in &selected {
        prefix.push(prefix.last().map_or(end, |&old: &T| old.max(end)));
    }
    for i in 0..s.len() {
        if mask[i] {
            continue;
        }
        assert!(s[i] < e[i], "empty rows must select themselves");
        let j = selected.partition_point(|&(start, _)| start < e[i]);
        assert!(
            j > 0 && prefix[j - 1] > s[i],
            "undominated original row {i}"
        );
    }
    mask.iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .fold((0, 0), |(cost, count), (i, _)| {
            (cost + w.map_or(1, |w| i128::from(w[i])), count + 1)
        })
}

fn known_optimum<T: Ord + Copy>(
    s: &[T],
    e: &[T],
    w: Option<&[i64]>,
    family: &str,
) -> Option<(i128, usize)> {
    let cost = |i| w.map_or(1, |w| i128::from(w[i]));
    if s.len() <= 12 {
        return (0..1usize << s.len())
            .filter(|&bits| {
                (0..s.len()).all(|i| {
                    bits >> i & 1 != 0
                        || (s[i] < e[i]
                            && (0..s.len()).any(|j| {
                                bits >> j & 1 != 0 && s[j] < e[j] && s[i] < e[j] && s[j] < e[i]
                            }))
                })
            })
            .map(|bits| {
                (0..s.len())
                    .filter(|i| bits >> i & 1 != 0)
                    .fold((0, 0), |(c, k), i| (c + cost(i), k + 1))
            })
            .min();
    }
    match family {
        "disjoint" | "touching" | "empty" | "sparse" => {
            Some(((0..s.len()).map(cost).sum(), s.len()))
        }
        "clique" | "nested" => Some(((0..s.len()).map(cost).min().unwrap(), 1)),
        "star" => {
            let hub = (0..s.len()).min_by_key(|&i| s[i]).unwrap();
            Some((cost(hub), 1).min((
                (0..s.len()).filter(|&i| i != hub).map(cost).sum(),
                s.len() - 1,
            )))
        }
        "path" if w.is_none_or(|w| w.iter().all(|&x| x == w[0])) => {
            let k = s.len().div_ceil(3);
            Some((cost(0) * k as i128, k))
        }
        _ => None,
    }
}

fn stats<T: Ord + Copy>(s: &[T], e: &[T]) -> (usize, usize) {
    let mut rows: Vec<_> = (0..s.len()).filter(|&i| s[i] < e[i]).collect();
    let count = rows.len();
    rows.sort_unstable_by(|&i, &j| s[j].cmp(&s[i]).then(e[i].cmp(&e[j])));
    let (mut end, mut m) = (None, 0);
    for i in rows {
        if end.is_none_or(|v| e[i] < v) {
            end = Some(e[i]);
            m += 1;
        }
    }
    (m, count)
}

struct Settings<'a> {
    samples: usize,
    warmups: usize,
    methods: &'a [&'a str],
}

fn measure<T: Ord + Copy>(
    file: &mut impl Write,
    s: &[T],
    e: &[T],
    w: Option<&[i64]>,
    label: &str,
    family: &str,
    settings: &Settings<'_>,
) {
    let expected = verify(s, e, w, &run(s, e, w, "production"));
    let known = known_optimum(s, e, w, family);
    if let Some(optimum) = known {
        assert_eq!(expected, optimum);
    }
    let evidence = if s.len() <= 12 {
        "graph_subsets"
    } else if known.is_some() {
        "analytic"
    } else {
        "candidate_agreement"
    };
    let (m, nonempty) = stats(s, e);
    let methods: Vec<_> = settings
        .methods
        .iter()
        .copied()
        .filter(|&method| {
            (method != "quadratic" || s.len() <= 64)
                && (method != "greedy" || w.is_none_or(|w| w.iter().all(|&x| x == w[0])))
        })
        .map(|method| {
            let (mask, peak, count) = allocations::measure(|| run(s, e, w, method));
            assert_eq!(verify(s, e, w, &mask), expected, "{label}/{method}");
            for _ in 0..settings.warmups {
                drop(black_box(run(s, e, w, method)));
            }
            (method, peak, count)
        })
        .collect();
    for sample in 0..settings.samples {
        for offset in 0..methods.len() {
            let (method, peak, count) = methods[(sample + offset) % methods.len()];
            let tick = Instant::now();
            let mask = black_box(run(black_box(s), black_box(e), black_box(w), method));
            let ns = tick.elapsed().as_nanos();
            assert_eq!(verify(s, e, w, &mask), expected, "{label}/{method}");
            writeln!(
                file,
                "{label},{},{m},{nonempty},{method},{sample},{ns},{peak},{count},{},{},{evidence}",
                s.len(),
                expected.0,
                expected.1
            )
            .unwrap();
        }
    }
    file.flush().unwrap();
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench");
    let value = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let sizes: Vec<usize> = value("DOMINATION_SIZES", "0,1,8,32,1000,10000,100000,1000000")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let seeds: Vec<u64> = value("DOMINATION_SEEDS", "7,41")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let methods = value(
        "DOMINATION_METHODS",
        &format!("production,{}", candidates::METHODS.join(",")),
    );
    let methods: Vec<_> = methods.split(',').collect();
    let settings = Settings {
        samples: value("DOMINATION_SAMPLES", "5").parse().unwrap(),
        warmups: value("DOMINATION_WARMUPS", "2").parse().unwrap(),
        methods: &methods,
    };
    assert!(settings.samples > 0 && settings.warmups > 0);
    let filter = value("DOMINATION_CASES", "");
    let dtypes = value("DOMINATION_DTYPES", "i64,u64");
    let path = value(
        "DOMINATION_CSV",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/minimum-cost-dominating-set.csv"
        ),
    );
    let mut file = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("DOMINATION_CSV must name a new file"),
    );
    writeln!(file,"dtype,family,order,costs,seed,n,m,candidates,method,sample,total_ns,peak_bytes,allocations,optimum_cost,optimum_count,evidence").unwrap();
    for &(family, order, mode, million) in CASES {
        let case = format!("{family}-{order}-{mode}");
        if !filter.is_empty() && !filter.split(',').any(|x| x == case) {
            continue;
        }
        for &n in &sizes {
            if n >= 1_000_000 && !million {
                continue;
            }
            for &seed in &seeds {
                let (s, e, c) = dataset(family, order, mode, n, seed);
                let w = (mode != "units").then_some(c.as_slice());
                if dtypes.split(',').any(|x| x == "i64") {
                    measure(
                        &mut file,
                        &s,
                        &e,
                        w,
                        &format!("i64,{family},{order},{mode},{seed}"),
                        family,
                        &settings,
                    );
                }
                if dtypes.split(',').any(|x| x == "u64") {
                    let us: Vec<_> = s
                        .iter()
                        .map(|&x| (i128::from(x) - i128::from(i64::MIN)) as u64)
                        .collect();
                    let ue: Vec<_> = e
                        .iter()
                        .map(|&x| (i128::from(x) - i128::from(i64::MIN)) as u64)
                        .collect();
                    measure(
                        &mut file,
                        &us,
                        &ue,
                        w,
                        &format!("u64,{family},{order},{mode},{seed}"),
                        family,
                        &settings,
                    );
                }
            }
            eprintln!("verified and measured {case}: {n}");
        }
    }
}
