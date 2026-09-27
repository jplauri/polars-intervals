//! Benchmark/reference implementations, never linked into the library.
use intervals_core::IntervalError;
use std::mem::size_of;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Measurement {
    pub counts: Vec<usize>,
    pub compression: Duration,
    pub sorting: Duration,
    pub sweep: Duration,
    // Peak live Vec capacity bytes, including output but excluding inputs.
    pub bytes: usize,
}

pub trait Counter {
    fn new(n: usize) -> Self;
    fn add(&mut self, index: usize);
    fn prefix(&self, end: usize) -> usize;
    fn bytes(&self) -> usize;
}

pub struct Fenwick(Vec<usize>);
impl Counter for Fenwick {
    fn new(n: usize) -> Self {
        Self(vec![0; n + 1])
    }
    fn add(&mut self, index: usize) {
        let mut i = index + 1;
        while i < self.0.len() {
            self.0[i] += 1;
            i += i.isolate_lowest_one();
        }
    }
    fn prefix(&self, mut end: usize) -> usize {
        let mut total = 0;
        while end > 0 {
            total += self.0[end];
            end &= end - 1;
        }
        total
    }
    fn bytes(&self) -> usize {
        self.0.capacity() * size_of::<usize>()
    }
}

pub struct Segment(Vec<usize>, usize);
impl Counter for Segment {
    fn new(n: usize) -> Self {
        Self(vec![0; 2 * n], n)
    }
    fn add(&mut self, index: usize) {
        let mut i = index + self.1;
        while i > 0 {
            self.0[i] += 1;
            i /= 2;
        }
    }
    fn prefix(&self, end: usize) -> usize {
        let (mut left, mut right) = (self.1, self.1 + end);
        let mut total = 0;
        while left < right {
            if left % 2 == 1 {
                total += self.0[left];
                left += 1;
            }
            if right % 2 == 1 {
                right -= 1;
                total += self.0[right];
            }
            left /= 2;
            right /= 2;
        }
        total
    }
    fn bytes(&self) -> usize {
        self.0.capacity() * size_of::<usize>()
    }
}

fn validate<T: Ord>(starts: &[T], ends: &[T]) -> Result<(), IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    for (index, (start, end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
    }
    Ok(())
}

pub fn packed<T: Ord + Copy, C: Counter>(
    starts: &[T],
    ends: &[T],
) -> Result<Measurement, IntervalError> {
    validate(starts, ends)?;
    let mut records: Vec<_> = starts
        .iter()
        .copied()
        .zip(ends.iter().copied())
        .enumerate()
        .map(|(i, (s, e))| (s, e, i))
        .collect();
    let mut out = Measurement::default();
    let tick = Instant::now();
    let mut coords = ends.to_vec();
    coords.sort_unstable();
    coords.dedup();
    out.counts = ends
        .iter()
        .map(|e| coords.binary_search(e).unwrap())
        .collect();
    out.compression = tick.elapsed();
    let tick = Instant::now();
    records.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    out.sorting = tick.elapsed();
    let tick = Instant::now();
    let mut tree = C::new(coords.len());
    for group in records.chunk_by(|a, b| a.0 == b.0) {
        for &(_, _, i) in group {
            tree.add(out.counts[i]);
        }
        for &(_, _, i) in group {
            out.counts[i] = tree.prefix(out.counts[i] + 1) - 1;
        }
    }
    out.sweep = tick.elapsed();
    out.bytes = records.capacity() * size_of::<(T, T, usize)>()
        + coords.capacity() * size_of::<T>()
        + out.counts.capacity() * size_of::<usize>()
        + tree.bytes();
    Ok(out)
}

pub fn indirect<T: Ord + Copy>(starts: &[T], ends: &[T]) -> Result<Measurement, IntervalError> {
    validate(starts, ends)?;
    let mut indices: Vec<_> = (0..starts.len()).collect();
    let mut out = Measurement::default();
    let tick = Instant::now();
    let mut coords = ends.to_vec();
    coords.sort_unstable();
    coords.dedup();
    out.counts = ends
        .iter()
        .map(|e| coords.binary_search(e).unwrap())
        .collect();
    out.compression = tick.elapsed();
    let tick = Instant::now();
    indices.sort_unstable_by(|&a, &b| {
        starts[b]
            .cmp(&starts[a])
            .then(ends[a].cmp(&ends[b]))
            .then(a.cmp(&b))
    });
    out.sorting = tick.elapsed();
    let tick = Instant::now();
    let mut tree = Fenwick::new(coords.len());
    for group in indices.chunk_by(|&a, &b| starts[a] == starts[b]) {
        for &i in group {
            tree.add(out.counts[i]);
        }
        for &i in group {
            out.counts[i] = tree.prefix(out.counts[i] + 1) - 1;
        }
    }
    out.sweep = tick.elapsed();
    out.bytes = indices.capacity() * size_of::<usize>()
        + coords.capacity() * size_of::<T>()
        + out.counts.capacity() * size_of::<usize>()
        + tree.bytes();
    Ok(out)
}

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

// Fixed, portable PRNG; no benchmark dependency or OS-specific random seed.
pub fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

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
        for i in (1..n).rev() {
            let j = (random(&mut seed) % (i + 1) as u64) as usize;
            rows.swap(i, j);
        }
    }
    rows.into_iter().unzip()
}
