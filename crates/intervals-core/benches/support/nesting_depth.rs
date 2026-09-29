//! Benchmark workloads and a quadratic oracle, never linked into the library.
#[path = "random.rs"]
mod random;
use random::{random, shuffle};

/// Independent quadratic DP: establish topological order using starts and ends,
/// then test the literal strict predicate against every preceding interval.
pub fn naive<T: Ord>(starts: &[T], ends: &[T]) -> Vec<usize> {
    let mut order: Vec<_> = (0..starts.len()).collect();
    order.sort_unstable_by(|&a, &b| starts[a].cmp(&starts[b]).then(ends[b].cmp(&ends[a])));
    let mut depths = vec![0; starts.len()];
    for &j in &order {
        for &i in &order {
            if starts[i] <= starts[j]
                && ends[j] <= ends[i]
                && (starts[i] < starts[j] || ends[j] < ends[i])
            {
                depths[j] = depths[j].max(depths[i] + 1);
            }
        }
    }
    depths
}

pub const SCENARIOS: [&str; 20] = [
    "disjoint",
    "crossing",
    "chain",
    "balanced",
    "identical",
    "duplicate_chain",
    "equal_starts",
    "equal_ends",
    "empty",
    "mixed",
    "sparse",
    "dense",
    "broad",
    "overlap_little_containment",
    "depth_4",
    "depth_64",
    "sorted",
    "reverse",
    "nearly",
    "shuffled",
];

pub fn dataset(name: &str, n: usize) -> (Vec<i64>, Vec<i64>) {
    let mut seed = 42;
    let len = n as i64;
    let mut rows: Vec<_> = (0..n)
        .map(|i| {
            let k = i as i64;
            match name {
                "disjoint" => (3 * k, 3 * k + 1),
                "crossing" => (k, k + len + 1),
                "overlap_little_containment" => {
                    // Long, mostly crossing intervals; each ten-row block has a
                    // single reversed end pair, producing shallow containment.
                    let end = k + len + 4 + i64::from(i % 10 == 0) * 2;
                    (k, end)
                }
                "chain" => (k, 2 * len - k),
                "balanced" => {
                    let node = i + 1;
                    let level = node.ilog2();
                    let span = 1i64 << (usize::BITS - n.max(1).leading_zeros() - level);
                    let position = (node - (1 << level)) as i64;
                    (position * span, (position + 1) * span)
                }
                "identical" => (1, 5),
                "duplicate_chain" => (k / 8, 2 * len - k / 8),
                "equal_starts" => (0, k + 1),
                "equal_ends" => (k, len),
                "empty" => (k % 64, k % 64),
                "mixed" => (k % 64, k % 64 + k % 5),
                "sparse" => {
                    let s = (random(&mut seed) % (4 * n.max(1)) as u64) as i64;
                    (s, s + (random(&mut seed) % 9) as i64)
                }
                "dense" | "sorted" | "reverse" | "nearly" | "shuffled" => {
                    let s = (random(&mut seed) % 4096) as i64;
                    (s, s + (random(&mut seed) % 4096) as i64)
                }
                "broad" if i % 10 == 0 => (-k, 4 * len + k),
                "broad" => (3 * k, 3 * k + 1),
                "depth_4" | "depth_64" => {
                    let levels = if name == "depth_4" { 5 } else { 65 };
                    let offset = k / levels * 4 * levels;
                    let depth = k % levels;
                    (offset + depth, offset + 2 * levels - depth)
                }
                _ => panic!("unknown scenario: {name}"),
            }
        })
        .collect();
    if ["sorted", "reverse", "nearly", "shuffled"].contains(&name) {
        rows.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        match name {
            "reverse" => rows.reverse(),
            "nearly" => {
                for i in (1..n).step_by(100) {
                    rows.swap(i - 1, i);
                }
            }
            "shuffled" => shuffle(&mut rows, &mut seed),
            _ => (),
        }
    } else if name != "identical" {
        shuffle(&mut rows, &mut seed);
    }
    rows.into_iter().unzip()
}
