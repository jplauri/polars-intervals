//! Complete-call experiment; see CLIQUE_* environment variables in main.
//! CLIQUE_CSV must name a new file, including when rerunning the default target.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/clique.rs"]
mod candidates;

use std::{collections::BTreeMap, hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

#[derive(Clone, Copy)]
struct Case {
    family: &'static str,
    order: &'static str,
    weights: &'static str,
    million: bool,
}

const CASES: &[Case] = &[
    Case {
        family: "disjoint",
        order: "start",
        weights: "units",
        million: true,
    },
    Case {
        family: "disjoint",
        order: "shuffled",
        weights: "positive",
        million: true,
    },
    Case {
        family: "touching",
        order: "reverse",
        weights: "units",
        million: false,
    },
    Case {
        family: "low2",
        order: "start",
        weights: "units",
        million: true,
    },
    Case {
        family: "low2",
        order: "start",
        weights: "ones",
        million: false,
    },
    Case {
        family: "low2",
        order: "shuffled",
        weights: "positive",
        million: true,
    },
    Case {
        family: "low8",
        order: "partial",
        weights: "mixed",
        million: false,
    },
    Case {
        family: "low8",
        order: "end",
        weights: "positive",
        million: false,
    },
    Case {
        family: "ragged",
        order: "start",
        weights: "positive",
        million: false,
    },
    Case {
        family: "ragged",
        order: "shuffled",
        weights: "units",
        million: false,
    },
    Case {
        family: "moderate",
        order: "shuffled",
        weights: "units",
        million: true,
    },
    Case {
        family: "moderate",
        order: "shuffled",
        weights: "ones",
        million: false,
    },
    Case {
        family: "moderate",
        order: "shuffled",
        weights: "skewed",
        million: false,
    },
    Case {
        family: "full",
        order: "shuffled",
        weights: "positive",
        million: true,
    },
    Case {
        family: "full",
        order: "start",
        weights: "units",
        million: false,
    },
    Case {
        family: "nested",
        order: "start",
        weights: "units",
        million: true,
    },
    Case {
        family: "nested",
        order: "end",
        weights: "positive",
        million: false,
    },
    Case {
        family: "staircase",
        order: "reverse",
        weights: "skewed",
        million: false,
    },
    Case {
        family: "duplicate",
        order: "shuffled",
        weights: "units",
        million: true,
    },
    Case {
        family: "duplicate",
        order: "reverse",
        weights: "ones",
        million: false,
    },
    Case {
        family: "repeated",
        order: "shuffled",
        weights: "positive",
        million: true,
    },
    Case {
        family: "repeated",
        order: "partial",
        weights: "ones",
        million: false,
    },
    Case {
        family: "blocks",
        order: "shuffled",
        weights: "mixed",
        million: false,
    },
    Case {
        family: "empty",
        order: "start",
        weights: "units",
        million: true,
    },
    Case {
        family: "empty",
        order: "shuffled",
        weights: "positive",
        million: false,
    },
    Case {
        family: "many_empty",
        order: "partial",
        weights: "positive",
        million: false,
    },
    Case {
        family: "sparse",
        order: "shuffled",
        weights: "units",
        million: true,
    },
    Case {
        family: "moderate",
        order: "reverse",
        weights: "nonpositive",
        million: false,
    },
];

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn dataset(case: Case, n: usize, seed: u64) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let mut rng = seed;
    let mut rows: Vec<_> = (0..n)
        .map(|index| {
            let i = index as i64;
            let n = n as i64;
            let (s, e) = match case.family {
                "disjoint" => (i * 3, i * 3 + 1),
                "touching" => (i, i + 1),
                "low2" => (i, i + 2),
                "low8" => (i, i + 8),
                "ragged" => (i, i + (random(&mut rng) % 128 + 1) as i64),
                "moderate" => (i, i + 64),
                "full" => (i, n + i + 1),
                "nested" => (i, 2 * n - i),
                "staircase" => (i * 2, i * 2 + 3),
                "duplicate" => ((i / 32) * 3, (i / 32) * 3 + 2),
                "repeated" => (i % 32, i % 32 + 16),
                "blocks" => ((i / 32) * 1000 + i % 32, (i / 32) * 1000 + i % 32 + 16),
                "empty" => (i % 16, i % 16),
                "many_empty" => (i, i + i64::from(i % 5 == 0) * 32),
                "sparse" => {
                    if index == n as usize - 1 {
                        (i64::MAX - 1, i64::MAX)
                    } else {
                        (
                            i64::MIN + i * 1_000_000_000_000,
                            i64::MIN + i * 1_000_000_000_000 + 1,
                        )
                    }
                }
                _ => unreachable!(),
            };
            let weight = match case.weights {
                "units" | "ones" => 1,
                "positive" => (random(&mut rng) % 17 + 1) as i64,
                "skewed" => {
                    if random(&mut rng).is_multiple_of(100) {
                        1_000_000
                    } else {
                        1
                    }
                }
                "mixed" => (random(&mut rng) % 11) as i64 - 8,
                "nonpositive" => -((random(&mut rng) % 10) as i64),
                _ => unreachable!(),
            };
            (s, e, weight)
        })
        .collect();
    match case.order {
        "start" => rows.sort_unstable_by_key(|r| r.0),
        "end" => rows.sort_unstable_by_key(|r| r.1),
        "reverse" => rows.sort_unstable_by_key(|r| std::cmp::Reverse(r.0)),
        "shuffled" => {
            for i in (1..n).rev() {
                rows.swap(i, (random(&mut rng) as usize) % (i + 1));
            }
        }
        "partial" => {
            rows.sort_unstable_by_key(|r| r.0);
            for chunk in rows.chunks_mut(32) {
                chunk.reverse();
            }
        }
        _ => unreachable!(),
    }
    let mut starts = Vec::with_capacity(n);
    let mut ends = Vec::with_capacity(n);
    let mut weights = Vec::with_capacity(n);
    for (s, e, w) in rows {
        starts.push(s);
        ends.push(e);
        weights.push(w);
    }
    (starts, ends, weights)
}

fn run<T: Ord + Copy, W: Copy>(s: &[T], e: &[T], w: Option<&[W]>, method: &str) -> Vec<bool>
where
    i128: From<W>,
{
    if method == "production" {
        w.map_or_else(
            || intervals_core::max_clique(s, e),
            |w| intervals_core::max_weight_clique(s, e, w),
        )
        .unwrap()
    } else {
        candidates::run(s, e, w, method).unwrap()
    }
}

// Independent full-size optimum cross-check: aggregate sparse coordinate deltas
// in a comparison tree. Benchmark weights are bounded, so both intermediate
// deltas and objectives are representable. This reference is never timed.
fn oracle<T: Ord + Copy, W: Copy>(s: &[T], e: &[T], w: Option<&[W]>) -> i128
where
    i128: From<W>,
{
    let mut changes = BTreeMap::<T, i128>::new();
    let mut best = 0;
    for i in 0..s.len() {
        let weight = w.map_or(1, |w| i128::from(w[i]));
        if weight > 0 {
            if s[i] == e[i] {
                best = best.max(weight);
            } else {
                *changes.entry(s[i]).or_default() += weight;
                *changes.entry(e[i]).or_default() -= weight;
            }
        }
    }
    let mut active = 0;
    for delta in changes.values() {
        active += delta;
        best = best.max(active);
    }
    best
}

// Linear feasibility validation for million-row masks, including isolated empties.
fn verify<T: Ord + Copy, W: Copy>(s: &[T], e: &[T], w: Option<&[W]>, mask: &[bool], optimum: i128)
where
    i128: From<W>,
{
    assert_eq!(s.len(), mask.len());
    let mut lo = None;
    let mut hi = None;
    let (mut count, mut empty, mut total) = (0, false, 0i128);
    for i in 0..s.len() {
        if mask[i] {
            let weight = w.map_or(1, |w| i128::from(w[i]));
            assert!(weight > 0);
            count += 1;
            empty |= s[i] == e[i];
            lo = Some(lo.map_or(s[i], |old: T| old.max(s[i])));
            hi = Some(hi.map_or(e[i], |old: T| old.min(e[i])));
            total += weight;
        }
    }
    assert_eq!(total, optimum);
    if empty {
        assert_eq!(count, 1);
    } else if count > 0 {
        assert!(lo.unwrap() < hi.unwrap());
    }
}

struct Settings<'a> {
    samples: usize,
    warmups: usize,
    methods: &'a [&'a str],
}

fn measure<T: Ord + Copy, W: Copy>(
    file: &mut impl Write,
    data: (&[T], &[T], Option<&[W]>),
    label: &str,
    settings: &Settings<'_>,
) where
    i128: From<W>,
{
    let (s, e, w) = data;
    let optimum = oracle(s, e, w);
    let expected = run(s, e, w, "production");
    verify(s, e, w, &expected, optimum);
    let methods: Vec<_> = settings
        .methods
        .iter()
        .copied()
        .filter(|&method| method != "quadratic" || s.len() <= 64)
        .map(|method| {
            let (mask, peak, count) = allocations::measure(|| run(s, e, w, method));
            verify(s, e, w, &mask, optimum);
            assert_eq!(mask, expected, "{label}/{method}");
            for _ in 0..settings.warmups {
                black_box(run(s, e, w, method));
            }
            (method, peak, count)
        })
        .collect();
    // Batch tiny calls; output destruction is included for every candidate.
    let iterations = if s.len() <= 64 {
        256
    } else if s.len() <= 1000 {
        8
    } else {
        1
    };
    for sample in 0..settings.samples {
        for offset in 0..methods.len() {
            let (method, peak, count) = methods[(sample + offset) % methods.len()];
            let tick = Instant::now();
            for _ in 0..iterations {
                drop(black_box(run(
                    black_box(s),
                    black_box(e),
                    black_box(w),
                    method,
                )));
            }
            let elapsed = tick.elapsed().as_nanos();
            writeln!(
                file,
                "{label},{},{method},{sample},{iterations},{elapsed},{},{peak},{count},{optimum}",
                s.len(),
                elapsed / iterations
            )
            .unwrap();
        }
    }
    file.flush().unwrap();
}

fn measure_weights<T: Ord + Copy>(
    file: &mut impl Write,
    data: (&[T], &[T], Option<&[i64]>),
    label: &str,
    settings: &Settings<'_>,
    weight_dtypes: &str,
) {
    let (s, e, w) = data;
    if w.is_none() || weight_dtypes.split(',').any(|dtype| dtype == "i64") {
        let dtype = if w.is_none() { "unit" } else { "i64" };
        measure(file, data, &format!("{label},{dtype}"), settings);
    }
    if let Some(w) = w
        && weight_dtypes.split(',').any(|dtype| dtype == "i128")
    {
        let wide: Vec<_> = w.iter().map(|&x| i128::from(x)).collect();
        measure(
            file,
            (s, e, Some(wide.as_slice())),
            &format!("{label},i128"),
            settings,
        );
    }
}

#[allow(clippy::assertions_on_constants)]
fn main() {
    assert!(!cfg!(debug_assertions), "use cargo bench (release mode)");
    let value = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let sizes: Vec<usize> = value("CLIQUE_SIZES", "0,1,8,32,64,1000,10000,100000,1000000")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let seeds: Vec<u64> = value("CLIQUE_SEEDS", "7,41")
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect();
    let methods = value(
        "CLIQUE_METHODS",
        &format!("production,{}", candidates::METHODS.join(",")),
    );
    let methods: Vec<_> = methods.split(',').collect();
    let samples = value("CLIQUE_SAMPLES", "5").parse().unwrap();
    let warmups = value("CLIQUE_WARMUPS", "2").parse().unwrap();
    assert!(
        samples > 0 && warmups > 0,
        "samples and warmups must be positive"
    );
    assert!(
        methods
            .iter()
            .all(|method| *method == "production" || candidates::METHODS.contains(method))
    );
    let filter = value("CLIQUE_CASES", "");
    let path = value(
        "CLIQUE_CSV",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/max-weight-clique.csv"
        ),
    );
    let dtypes = value("CLIQUE_DTYPES", "i64,u64");
    let weight_dtypes = value("CLIQUE_WEIGHT_DTYPES", "i64,i128");
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("CLIQUE_CSV must name a new file; preserve previous raw runs");
    let mut file = std::io::BufWriter::new(output);
    writeln!(file, "dtype,family,order,weights,seed,weight_dtype,n,method,sample,iterations,batch_ns,total_ns,peak_bytes,allocations,optimum").unwrap();
    eprintln!(
        "layouts bytes: endpoint i64={}, index={}, event unit={}, event weighted={}, stream weighted={}, heap entry={}",
        size_of::<i64>(),
        size_of::<usize>(),
        size_of::<(i64, bool)>(),
        size_of::<(i64, i128)>(),
        size_of::<(i64, i64)>(),
        size_of::<(i64, usize)>()
    );
    eprintln!(
        "scope: validation, preparation, optimization, mask and destruction; warmups={warmups}; samples={samples}; quadratic omitted above n=64; million cases focused"
    );
    for &case in CASES {
        let case_name = format!("{}-{}-{}", case.family, case.order, case.weights);
        if !filter.is_empty() && !filter.split(',').any(|name| name == case_name) {
            continue;
        }
        for &n in &sizes {
            if n >= 1_000_000 && !case.million {
                continue;
            }
            for &seed in &seeds {
                let (s, e, weights) = dataset(case, n, seed);
                let w = (case.weights != "units").then_some(weights.as_slice());
                let settings = Settings {
                    samples,
                    warmups,
                    methods: &methods,
                };
                if dtypes.split(',').any(|dtype| dtype == "i64") {
                    measure_weights(
                        &mut file,
                        (&s, &e, w),
                        &format!("i64,{},{},{},{seed}", case.family, case.order, case.weights),
                        &settings,
                        &weight_dtypes,
                    );
                }
                if dtypes.split(',').any(|dtype| dtype == "u64") {
                    // Order isomorphism spans the unsigned range, preserving all
                    // geometry including sparse extrema without float conversion.
                    let us: Vec<_> = s
                        .iter()
                        .map(|&x| (i128::from(x) - i128::from(i64::MIN)) as u64)
                        .collect();
                    let ue: Vec<_> = e
                        .iter()
                        .map(|&x| (i128::from(x) - i128::from(i64::MIN)) as u64)
                        .collect();
                    measure_weights(
                        &mut file,
                        (&us, &ue, w),
                        &format!("u64,{},{},{},{seed}", case.family, case.order, case.weights),
                        &settings,
                        &weight_dtypes,
                    );
                }
            }
            eprintln!("verified and measured {case_name}: {n}");
        }
    }
}
