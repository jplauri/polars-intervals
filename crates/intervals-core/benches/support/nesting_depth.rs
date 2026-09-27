//! Exact benchmark candidates, deliberately separate from the production kernel.
#[path = "random.rs"]
mod random;
use intervals_core::IntervalError;
pub use random::{random, shuffle};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Measurement {
    pub depths: Vec<usize>,
    pub validation: Duration,
    pub preparation: Duration,
    pub sorting: Duration,
    pub compression: Duration,
    pub output_init: Duration,
    // Row-aligned scatter is fused into the DP, so it is included here.
    pub dp_and_scatter: Duration,
}

pub trait MaxTree {
    fn new(n: usize) -> Self;
    fn update(&mut self, rank: usize, depth: usize);
    fn prefix(&self, exclusive_end: usize) -> Option<usize>;
}

pub struct Fenwick(Vec<usize>);
impl MaxTree for Fenwick {
    fn new(n: usize) -> Self {
        Self(vec![0; n + 1])
    }
    fn update(&mut self, rank: usize, depth: usize) {
        let mut i = rank + 1;
        while i < self.0.len() {
            // Zero means absent; a stored value is depth + 1.
            self.0[i] = self.0[i].max(depth + 1);
            i += i.isolate_lowest_one();
        }
    }
    fn prefix(&self, mut exclusive_end: usize) -> Option<usize> {
        let mut best = 0;
        while exclusive_end != 0 {
            best = best.max(self.0[exclusive_end]);
            exclusive_end &= exclusive_end - 1;
        }
        best.checked_sub(1)
    }
}

pub struct Segment(Vec<usize>, usize);
impl MaxTree for Segment {
    fn new(n: usize) -> Self {
        Self(vec![0; 2 * n], n)
    }
    fn update(&mut self, rank: usize, depth: usize) {
        let mut i = self.1 + rank;
        self.0[i] = self.0[i].max(depth + 1);
        while i > 1 {
            i /= 2;
            self.0[i] = self.0[2 * i].max(self.0[2 * i + 1]);
        }
    }
    fn prefix(&self, exclusive_end: usize) -> Option<usize> {
        let (mut left, mut right) = (self.1, self.1 + exclusive_end);
        let mut best = 0;
        while left < right {
            if left % 2 == 1 {
                best = best.max(self.0[left]);
                left += 1;
            }
            if right % 2 == 1 {
                right -= 1;
                best = best.max(self.0[right]);
            }
            left /= 2;
            right /= 2;
        }
        best.checked_sub(1)
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

pub fn compressed_ranks<T: Ord + Copy>(ends: &[T]) -> (Vec<T>, Vec<usize>) {
    let mut coordinates = ends.to_vec();
    coordinates.sort_unstable();
    coordinates.dedup();
    let ranks = ends
        .iter()
        .map(|e| coordinates.len() - 1 - coordinates.binary_search(e).unwrap())
        .collect();
    (coordinates, ranks)
}

pub fn packed<T: Ord + Copy, C: MaxTree>(
    starts: &[T],
    ends: &[T],
    detect_sorted: bool,
) -> Result<Measurement, IntervalError> {
    let mut result = Measurement::default();
    let tick = Instant::now();
    validate(starts, ends)?;
    result.validation = tick.elapsed();
    let tick = Instant::now();
    let mut records: Vec<_> = starts
        .iter()
        .copied()
        .zip(ends.iter().copied())
        .enumerate()
        .map(|(i, (s, e))| (s, e, i))
        .collect();
    result.preparation = tick.elapsed();
    let tick = Instant::now();
    let cmp = |a: &(T, T, usize), b: &(T, T, usize)| {
        a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2))
    };
    if !detect_sorted || !records.is_sorted_by(|a, b| cmp(a, b).is_le()) {
        records.sort_unstable_by(cmp);
    }
    result.sorting = tick.elapsed();
    let tick = Instant::now();
    // Reuse the row-aligned rank vector as the output once a row is processed.
    let (coordinates, ranks) = compressed_ranks(ends);
    result.depths = ranks;
    let coordinate_count = coordinates.len();
    drop(coordinates);
    result.compression = tick.elapsed();
    let tick = Instant::now();
    let mut tree = C::new(coordinate_count);
    for group in records.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
        let rank = result.depths[group[0].2];
        let depth = tree.prefix(rank + 1).map_or(0, |d| d + 1);
        for &(_, _, index) in group {
            result.depths[index] = depth;
        }
        // All equal geometries receive their depth before this single update.
        tree.update(rank, depth);
    }
    result.dp_and_scatter = tick.elapsed();
    Ok(result)
}

pub fn indirect<T: Ord + Copy>(starts: &[T], ends: &[T]) -> Result<Measurement, IntervalError> {
    let mut result = Measurement::default();
    let tick = Instant::now();
    validate(starts, ends)?;
    result.validation = tick.elapsed();
    let tick = Instant::now();
    let mut indices: Vec<_> = (0..starts.len()).collect();
    result.preparation = tick.elapsed();
    let tick = Instant::now();
    indices.sort_unstable_by(|&a, &b| {
        starts[a]
            .cmp(&starts[b])
            .then(ends[b].cmp(&ends[a]))
            .then(a.cmp(&b))
    });
    result.sorting = tick.elapsed();
    let tick = Instant::now();
    let (coordinates, ranks) = compressed_ranks(ends);
    result.depths = ranks;
    let coordinate_count = coordinates.len();
    drop(coordinates);
    result.compression = tick.elapsed();
    let tick = Instant::now();
    let mut tree = Fenwick::new(coordinate_count);
    for group in indices.chunk_by(|&a, &b| starts[a] == starts[b] && ends[a] == ends[b]) {
        let rank = result.depths[group[0]];
        let depth = tree.prefix(rank + 1).map_or(0, |d| d + 1);
        for &index in group {
            result.depths[index] = depth;
        }
        tree.update(rank, depth);
    }
    result.dp_and_scatter = tick.elapsed();
    Ok(result)
}

pub fn frontier<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    detect_sorted: bool,
    append_fast: bool,
) -> Result<Measurement, IntervalError> {
    let mut result = Measurement::default();
    let tick = Instant::now();
    validate(starts, ends)?;
    result.validation = tick.elapsed();
    let tick = Instant::now();
    let mut records: Vec<_> = starts
        .iter()
        .copied()
        .zip(ends.iter().copied())
        .enumerate()
        .map(|(i, (s, e))| (s, e, i))
        .collect();
    result.preparation = tick.elapsed();
    let tick = Instant::now();
    let cmp = |a: &(T, T, usize), b: &(T, T, usize)| {
        a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2))
    };
    if !detect_sorted || !records.is_sorted_by(|a, b| cmp(a, b).is_le()) {
        records.sort_unstable_by(cmp);
    }
    result.sorting = tick.elapsed();
    let tick = Instant::now();
    result.depths = vec![0; starts.len()];
    result.output_init = tick.elapsed();
    let tick = Instant::now();
    let mut tails = Vec::new();
    for group in records.chunk_by(|a, b| a.0 == b.0 && a.1 == b.1) {
        let end = group[0].1;
        // tails[d] is the greatest last end of any chain of cardinality d+1.
        // Tails are nonincreasing. Extending the longest eligible chain gives
        // this geometry's depth; updating once prevents duplicate chaining.
        let depth = if append_fast && tails.last().is_some_and(|&tail| tail >= end) {
            tails.len()
        } else {
            tails.partition_point(|&tail| tail >= end)
        };
        for &(_, _, index) in group {
            result.depths[index] = depth;
        }
        if depth == tails.len() {
            tails.push(end);
        } else {
            tails[depth] = end;
        }
    }
    result.dp_and_scatter = tick.elapsed();
    Ok(result)
}

pub const METHODS: [&str; 8] = [
    "A_packed_fenwick",
    "A_sorted_fenwick",
    "B_indirect_fenwick",
    "C_packed_segment",
    "D_frontier",
    "D_sorted_frontier",
    "D_append_frontier",
    "D_sorted_append_frontier",
];

pub fn run<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
    method: &str,
) -> Result<Measurement, IntervalError> {
    match method {
        "A_packed_fenwick" => packed::<_, Fenwick>(starts, ends, false),
        "A_sorted_fenwick" => packed::<_, Fenwick>(starts, ends, true),
        "B_indirect_fenwick" => indirect(starts, ends),
        "C_packed_segment" => packed::<_, Segment>(starts, ends, false),
        "D_frontier" => frontier(starts, ends, false, false),
        "D_sorted_frontier" => frontier(starts, ends, true, false),
        "D_append_frontier" => frontier(starts, ends, false, true),
        "D_sorted_append_frontier" => frontier(starts, ends, true, true),
        _ => panic!("unknown method: {method}"),
    }
}

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
