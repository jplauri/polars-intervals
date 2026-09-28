//! Benchmark workloads and a quadratic oracle, never linked into the library.
#[path = "random.rs"]
mod random;
use random::random;

pub fn naive<T: Ord>(starts: &[T], ends: &[T]) -> Vec<usize> {
    (0..starts.len())
        .map(|i| {
            (0..starts.len())
                .filter(|&j| i != j && starts[i] <= starts[j] && ends[j] <= ends[i])
                .count()
        })
        .collect()
}

pub const SCENARIOS: [&str; 14] = [
    "disjoint",
    "nested",
    "duplicates",
    "equal_starts",
    "equal_ends",
    "sparse",
    "dense",
    "broad",
    "crossing",
    "empty",
    "mixed",
    "sorted",
    "reverse",
    "shuffled",
];

pub fn dataset(name: &str, n: usize) -> (Vec<i64>, Vec<i64>) {
    let mut seed = 42;
    let mut rows: Vec<_> = (0..n)
        .map(|i| {
            let k = i as i64;
            let len = n as i64;
            match name {
                "disjoint" => (3 * k, 3 * k + 1),
                "nested" => (k, 2 * len - k),
                "duplicates" => (1, 5),
                "equal_starts" => (0, k + 1),
                "equal_ends" => (k, len),
                "sparse" => {
                    let s = (random(&mut seed) % (4 * n.max(1)) as u64) as i64;
                    (s, s + (random(&mut seed) % 9) as i64)
                }
                "dense" => {
                    let s = (random(&mut seed) % 1024) as i64;
                    (s, s + (random(&mut seed) % 1024) as i64)
                }
                "broad" if i % 10 == 0 => (-k, 4 * len + k),
                "broad" => (3 * k, 3 * k + 1),
                "crossing" => (k, k + len + 1),
                "empty" => (k % 64, k % 64),
                "mixed" => (k % 64, k % 64 + k % 5),
                "sorted" | "reverse" | "shuffled" => (len - k, len + k),
                _ => panic!("unknown scenario: {name}"),
            }
        })
        .collect();
    if name == "reverse" {
        rows.reverse();
    }
    if name == "shuffled" {
        random::shuffle(&mut rows, &mut seed);
    }
    rows.into_iter().unzip()
}
