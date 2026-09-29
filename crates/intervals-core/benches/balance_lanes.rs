//! Release core calls, separate from corpus-driven end-to-end Polars timing.
//! BALANCE_SIZES, BALANCE_SAMPLES, BALANCE_BUDGETS, BALANCE_FAMILIES,
//! BALANCE_CSV and BALANCE_QUALITY_CSV select the reproducible matrix/output.
//! Includes validation, minimum/seed construction, copying and allocation;
//! excludes fixture construction, independent checks and output destruction.
//! Every repair borrows the same ORIGINAL baseline. Memory is an untimed
//! requested-live-heap measurement, not RSS or tracked buffer capacity.
#[path = "support/allocations.rs"]
mod allocations;
mod support;

use intervals_core::{BalanceDiagnostics, assign_balanced_lanes_with_diagnostics, assign_lanes};
use std::{hint::black_box, io::Write, time::Instant};

#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

struct Outcome {
    lanes: Vec<u32>,
    diagnostics: Option<BalanceDiagnostics>,
    work: Option<u64>,
}

fn score(lanes: &[u32]) -> (usize, u128, usize, usize, usize) {
    let k = lanes.iter().max().map_or(0, |&v| v as usize + 1);
    let mut loads = vec![0usize; k];
    for &lane in lanes {
        loads[lane as usize] += 1;
    }
    let min = *loads.iter().min().unwrap_or(&0);
    let max = *loads.iter().max().unwrap_or(&0);
    (
        max - min,
        loads.iter().map(|&v| (v as u128).pow(2)).sum(),
        k,
        min,
        max,
    )
}

fn run(s: &[i64], e: &[i64], baseline: &[u32], method: &str, budget: u64) -> Outcome {
    let mut result = Outcome {
        lanes: Vec::new(),
        diagnostics: None,
        work: None,
    };
    if method == "baseline" {
        result.lanes = assign_lanes(s, e).unwrap();
    } else {
        let initial = (method == "repair").then_some(baseline);
        let measured = assign_balanced_lanes_with_diagnostics(s, e, initial, budget).unwrap();
        result.lanes = measured.lanes;
        result.work = Some(measured.diagnostics.work);
        result.diagnostics = Some(measured.diagnostics);
    }
    result
}

fn env_list(name: &str, default: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .split(',')
        .map(str::to_owned)
        .collect()
}

fn file(name: &str, fallback: &str) -> std::fs::File {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::env::var(name).unwrap_or_else(|_| fallback.to_owned()))
        .expect("choose a new output path; historical measurements are never overwritten")
}

fn main() {
    if cfg!(debug_assertions) {
        panic!("run with cargo bench (release)");
    }
    let mut timing = file("BALANCE_CSV", "balance-core-timings.csv");
    let mut quality = file("BALANCE_QUALITY_CSV", "balance-core-quality.csv");
    writeln!(timing, "family,order,n,omega,method,budget,sample,ns").unwrap();
    writeln!(quality,"family,order,n,nonempty,empty,omega,k,method,budget,min,max,D,Q,delta,D_minus_delta,starting_D,starting_Q,D_improvement,Q_improvement,work,pairs,flips,skips,stop_reason,requested_peak_heap_bytes,allocations,D_star,Q_star,optimum_status").unwrap();
    let sizes: Vec<usize> = env_list("BALANCE_SIZES", "1000,10000,100000")
        .iter()
        .map(|s| s.parse().unwrap())
        .collect();
    let budgets: Vec<u64> = env_list("BALANCE_BUDGETS", "10000,100000,1000000")
        .iter()
        .map(|s| s.parse().unwrap())
        .collect();
    let families = env_list(
        "BALANCE_FAMILIES",
        "low8,moderate128,clique,nearly_clique,late_clique,long_short,ties,duplicates,mixed_empty,stars",
    );
    let samples: usize = std::env::var("BALANCE_SAMPLES")
        .unwrap_or_else(|_| "7".to_owned())
        .parse()
        .unwrap();
    let methods = ["baseline", "repair", "balanced"];
    let mut state = 42;
    for n in sizes {
        for family in &families {
            let mut intervals: Vec<_> = (0..n as i64)
                .map(|i| match family.as_str() {
                    "low8" => (i, i + 8),
                    "moderate128" => (i, i + 128),
                    "clique" => (0, 1),
                    "nearly_clique" => {
                        if i < n as i64 * 3 / 4 {
                            (0, 1)
                        } else {
                            (2, 2)
                        }
                    }
                    "late_clique" => {
                        if i < n as i64 * 3 / 4 {
                            (i, i + 1)
                        } else {
                            (n as i64, n as i64 + 1)
                        }
                    }
                    "long_short" => {
                        if i % 32 == 0 {
                            (i, i + 64)
                        } else {
                            (i, i + 1)
                        }
                    }
                    "ties" => (i / 32 * 16, i / 32 * 16 + 16 + i % 4),
                    "duplicates" => (i / 16 * 4, i / 16 * 4 + 8),
                    "mixed_empty" => (i / 4, i / 4 + if i % 3 == 0 { 0 } else { 8 }),
                    "stars" => {
                        let root = i / 16 * 32;
                        if i % 16 == 0 {
                            (root, root + 32)
                        } else {
                            (root + (i % 16) * 2, root + (i % 16) * 2 + 1)
                        }
                    }
                    _ => panic!("unknown family {family}"),
                })
                .collect();
            for order in ["sorted", "shuffled"] {
                if order == "shuffled" {
                    support::shuffle(&mut intervals, &mut state);
                }
                let (s, e): (Vec<_>, Vec<_>) = intervals.iter().copied().unzip();
                let minimum = support::optimum(&s, &e);
                let baseline = assign_lanes(&s, &e).unwrap();
                let starting = score(&baseline);
                let empty = s.iter().zip(&e).filter(|(s, e)| s == e).count();
                let omega = if empty == n { 0 } else { minimum };
                let mut options = Vec::new();
                for method in methods {
                    let work_options: &[u64] = if method == "baseline" { &[0] } else { &budgets };
                    for &budget in work_options {
                        let (result, peak, allocations) =
                            allocations::measure(|| run(&s, &e, &baseline, method, budget));
                        support::verify(&s, &e, &result.lanes, minimum);
                        let (d, q, k, min, max) = score(&result.lanes);
                        assert!((d, q) <= (starting.0, starting.1));
                        let delta = usize::from(k > 0 && !n.is_multiple_of(k));
                        let work = result.work.map_or_else(String::new, |v| v.to_string());
                        let diagnostics = result.diagnostics.as_ref().map_or_else(
                            || ",,,unavailable".to_owned(),
                            |v| format!("{},{},{},{:?}", v.pairs, v.flips, v.skips, v.stop_reason),
                        );
                        let optimum = if d == delta {
                            format!("{d},{q},elementary_bound_attained")
                        } else {
                            ",,unknown".to_owned()
                        };
                        writeln!(quality,"{family},{order},{n},{},{empty},{omega},{k},{method},{budget},{min},{max},{d},{q},{delta},{},{},{},{},{},{work},{diagnostics},{peak},{allocations},{optimum}",n-empty,d-delta,starting.0,starting.1,starting.0 as i128-d as i128,starting.1 as i128-q as i128).unwrap();
                        options.push((method, budget, result.lanes));
                    }
                }
                for sample in 0..samples + 2 {
                    // Deterministic variation prevents a permanently privileged order.
                    for offset in 0..options.len() {
                        let (method, budget, expected) =
                            &options[(sample + offset) % options.len()];
                        let begin = Instant::now();
                        let result = black_box(run(
                            black_box(&s),
                            black_box(&e),
                            black_box(&baseline),
                            method,
                            *budget,
                        ));
                        let ns = begin.elapsed().as_nanos();
                        support::verify(&s, &e, &result.lanes, minimum);
                        assert_eq!(&result.lanes, expected);
                        if sample >= 2 {
                            writeln!(
                                timing,
                                "{family},{order},{n},{omega},{method},{budget},{},{ns}",
                                sample - 2
                            )
                            .unwrap();
                        }
                    }
                }
            }
        }
    }
}
