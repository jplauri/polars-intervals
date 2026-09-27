//! Benchmark-only exact alternatives. Not linked into the production library.
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

pub type Job = (i64, i64, i64);
pub type Segment = (i64, i64, usize);

#[derive(Default)]
pub struct Measurement {
    pub mask: Vec<bool>,
    pub normalization_ns: u128,
    pub preprocessing_ns: u128,
    pub timeline_ns: u128,
    pub solver_ns: u128,
    pub reconstruction_ns: u128,
    pub vertices: usize,
    pub edges: usize,
    pub augmentations: usize,
    pub components: usize,
}

pub fn normalize(profile: &[Segment], indirect: bool, limit: usize) -> Vec<Segment> {
    if indirect {
        let mut order: Vec<_> = (0..profile.len()).collect();
        order.sort_unstable_by_key(|&i| (profile[i].0, profile[i].1));
        normalize_sorted(order.into_iter().map(|i| profile[i]), limit)
    } else {
        let mut rows = profile.to_vec();
        rows.sort_unstable_by_key(|r| (r.0, r.1));
        normalize_sorted(rows.into_iter(), limit)
    }
}
fn normalize_sorted(rows: impl Iterator<Item = Segment>, limit: usize) -> Vec<Segment> {
    let mut result: Vec<Segment> = Vec::new();
    for (s, e, c) in rows {
        assert!(s <= e);
        if s == e {
            continue;
        }
        let c = c.min(limit);
        if let Some(&last) = result.last() {
            assert!(last.1 <= s);
            if last.1 < s {
                push(&mut result, (last.1, s, 0));
            }
        }
        push(&mut result, (s, e, c));
    }
    result
}
fn push(result: &mut Vec<Segment>, row: Segment) {
    if let Some(last) = result.last_mut()
        && last.1 == row.0
        && last.2 == row.2
    {
        last.1 = row.1;
    } else {
        result.push(row);
    }
}

struct Arc {
    to: usize,
    reverse: usize,
    residual: usize,
    cost: i128,
}
struct Flow {
    graph: Vec<Vec<Arc>>,
}
impl Flow {
    fn new(n: usize) -> Self {
        Self {
            graph: (0..n).map(|_| Vec::new()).collect(),
        }
    }
    fn edge(&mut self, u: usize, v: usize, capacity: usize, cost: i128) -> usize {
        let a = self.graph[u].len();
        let b = self.graph[v].len();
        self.graph[u].push(Arc {
            to: v,
            reverse: b,
            residual: capacity,
            cost,
        });
        self.graph[v].push(Arc {
            to: u,
            reverse: a,
            residual: 0,
            cost: -cost,
        });
        a
    }
    fn send(&mut self, source: usize, sink: usize, mut amount: usize) -> usize {
        let n = self.graph.len();
        // A/B initially form a forward DAG; conventional Bellman-Ford with an
        // artificial zero-cost source finds globally feasible potentials.
        // C starts with nonnegative costs, so this terminates in one pass.
        let mut h = vec![0i128; n];
        for pass in 0..n {
            let mut changed = false;
            for u in 0..n {
                for arc in &self.graph[u] {
                    if arc.residual > 0 && h[u] + arc.cost < h[arc.to] {
                        h[arc.to] = h[u] + arc.cost;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            assert!(pass + 1 < n, "initial negative cycle");
        }
        let mut iterations = 0;
        while amount > 0 {
            let mut d = vec![i128::MAX; n];
            let mut parent = vec![(usize::MAX, usize::MAX); n];
            let mut heap = BinaryHeap::new();
            d[source] = 0;
            heap.push(Reverse((0, source)));
            while let Some(Reverse((distance, u))) = heap.pop() {
                if distance != d[u] {
                    continue;
                }
                for (i, arc) in self.graph[u].iter().enumerate() {
                    if arc.residual == 0 {
                        continue;
                    }
                    let reduced = arc.cost + h[u] - h[arc.to];
                    assert!(reduced >= 0);
                    let next = distance + reduced;
                    if next < d[arc.to] {
                        d[arc.to] = next;
                        parent[arc.to] = (u, i);
                        heap.push(Reverse((next, arc.to)));
                    }
                }
            }
            assert_ne!(d[sink], i128::MAX);
            for v in 0..n {
                if d[v] != i128::MAX {
                    h[v] += d[v];
                }
            }
            let mut pushed = amount;
            let mut v = sink;
            while v != source {
                let (u, a) = parent[v];
                pushed = pushed.min(self.graph[u][a].residual);
                v = u;
            }
            v = sink;
            while v != source {
                let (u, a) = parent[v];
                let reverse = self.graph[u][a].reverse;
                self.graph[u][a].residual -= pushed;
                self.graph[v][reverse].residual += pushed;
                v = u;
            }
            amount -= pushed;
            iterations += 1;
        }
        iterations
    }
}

fn capacity_at(profile: &[Segment], t: i64) -> usize {
    let i = profile.partition_point(|r| r.1 <= t);
    profile.get(i).filter(|r| r.0 <= t).map_or(0, |r| r.2)
}

fn timeline(jobs: &[Job], rows: &[usize], profile: &[Segment]) -> (Vec<i64>, Vec<usize>) {
    let mut times: Vec<_> = rows.iter().flat_map(|&i| [jobs[i].0, jobs[i].1]).collect();
    let lo = *times.iter().min().unwrap();
    let hi = *times.iter().max().unwrap();
    times.extend(
        profile
            .iter()
            .flat_map(|r| [r.0, r.1])
            .filter(|&t| lo < t && t < hi),
    );
    times.sort_unstable();
    times.dedup();
    let capacities = times[..times.len() - 1]
        .iter()
        .map(|&t| capacity_at(profile, t))
        .collect();
    (times, capacities)
}

fn solve_component(jobs: &[Job], rows: &[usize], profile: &[Segment], model: &str) -> Measurement {
    let mut result = Measurement::default();
    if rows.is_empty() {
        return result;
    }
    result.components = 1;
    let begin = Instant::now();
    let (times, capacities) = timeline(jobs, rows, profile);
    let v = times.len();
    let mut flow = Flow::new(v + 2);
    let mut balance = vec![0i128; v];
    let mut links = Vec::new();
    match model {
        "C" => {
            for (j, &c) in capacities.iter().enumerate() {
                flow.edge(j + 1, j, c, 0);
            }
            // Saturate every negative job arc. Conservation is restored by
            // sending excess at ends to deficits at starts. The only initial
            // job residual arc is positive-cost rejection, hence no initial
            // negative cycle, and ordinary SSP is exact.
            for &i in rows {
                let (s, e, w) = jobs[i];
                let u = times.binary_search(&s).unwrap();
                let v = times.binary_search(&e).unwrap();
                let a = flow.edge(v, u, 1, i128::from(w));
                links.push((i, v, a));
                balance[v] += 1;
                balance[u] -= 1;
            }
        }
        "A" | "B" => {
            let k = capacities.iter().copied().max().unwrap_or(0);
            if model == "B" {
                // Fixed K s-t flow, represented by the fixed return arc
                // last -> first with lower=upper=K. Subtract timeline lower
                // bounds K-c, leaving residual upper bound c. Lower flow
                // contributes incoming-outgoing imbalance to each endpoint.
                balance[0] += k as i128;
                balance[v - 1] -= k as i128;
                for (j, &c) in capacities.iter().enumerate() {
                    let lower = k - c;
                    balance[j] -= lower as i128;
                    balance[j + 1] += lower as i128;
                    flow.edge(j, j + 1, k - lower, 0);
                }
            } else {
                for (j, &c) in capacities.iter().enumerate() {
                    balance[j] += c as i128;
                    balance[j + 1] -= c as i128;
                    flow.edge(j, j + 1, c, 0);
                }
            }
            for &i in rows {
                let (s, e, w) = jobs[i];
                let u = times.binary_search(&s).unwrap();
                let v = times.binary_search(&e).unwrap();
                let a = flow.edge(u, v, 1, -i128::from(w));
                links.push((i, u, a));
            }
        }
        _ => unreachable!(),
    }
    assert_eq!(balance.iter().sum::<i128>(), 0);
    let mut amount = 0;
    for (j, &b) in balance.iter().enumerate() {
        if b > 0 {
            flow.edge(v, j, b as usize, 0);
            amount += b as usize;
        } else if b < 0 {
            flow.edge(j, v + 1, (-b) as usize, 0);
        }
    }
    result.vertices = v + 2;
    result.edges = flow.graph.iter().map(Vec::len).sum::<usize>() / 2;
    result.timeline_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    result.augmentations = flow.send(v, v + 1, amount);
    result.solver_ns = begin.elapsed().as_nanos();
    let begin = Instant::now();
    // Local compact output; caller scatters using rows.
    result.mask = links
        .iter()
        .map(|&(_, u, a)| {
            if model == "C" {
                flow.graph[u][a].residual == 1
            } else {
                flow.graph[u][a].residual == 0
            }
        })
        .collect();
    result.reconstruction_ns = begin.elapsed().as_nanos();
    result
}

fn feasible(jobs: &[Job], rows: &[usize], profile: &[Segment]) -> bool {
    let mut events: Vec<_> = rows
        .iter()
        .flat_map(|&i| [(jobs[i].0, 1i64), (jobs[i].1, -1)])
        .collect();
    events.extend(profile.iter().flat_map(|r| [(r.0, 0), (r.1, 0)]));
    events.sort_unstable();
    let mut active = 0i64;
    let mut i = 0;
    while i < events.len() {
        let t = events[i].0;
        while i < events.len() && events[i].0 == t {
            active += events[i].1;
            i += 1;
        }
        if active as usize > capacity_at(profile, t) {
            return false;
        }
    }
    true
}

pub fn run(name: &str, jobs: &[Job], profile: &[Segment]) -> Measurement {
    let mut result = Measurement {
        mask: jobs.iter().map(|&(s, e, w)| s == e && w > 0).collect(),
        ..Measurement::default()
    };
    let begin = Instant::now();
    let normalized = normalize(profile, name == "indirect", jobs.len());
    result.normalization_ns = begin.elapsed().as_nanos();
    if name == "packed" || name == "indirect" {
        return result;
    }
    let begin = Instant::now();
    let mut rows: Vec<_> = (0..jobs.len())
        .filter(|&i| jobs[i].0 < jobs[i].1 && jobs[i].2 > 0)
        .collect();
    if name == "prefilter" {
        // Prefix measure of uncovered/zero-capacity segments: each job needs
        // only two binary searches after one compact timeline construction.
        if !rows.is_empty() {
            let (times, capacities) = timeline(jobs, &rows, &normalized);
            let mut zeros = vec![0usize; times.len()];
            for (j, &c) in capacities.iter().enumerate() {
                zeros[j + 1] = zeros[j] + usize::from(c == 0);
            }
            rows.retain(|&i| {
                zeros[times.binary_search(&jobs[i].0).unwrap()]
                    == zeros[times.binary_search(&jobs[i].1).unwrap()]
            });
        }
    }
    if name == "guarded" && feasible(jobs, &rows, &normalized) {
        for i in rows {
            result.mask[i] = true;
        }
        result.preprocessing_ns = begin.elapsed().as_nanos();
        return result;
    }
    let mut components = Vec::new();
    if name == "components" || name == "parallel" {
        rows.sort_unstable_by_key(|&i| (jobs[i].0, jobs[i].1, i));
        let mut start = 0;
        let mut end = i64::MIN;
        for (j, &i) in rows.iter().enumerate() {
            if j > start && jobs[i].0 >= end {
                components.push(start..j);
                start = j;
            }
            end = end.max(jobs[i].1);
        }
        if start < rows.len() {
            components.push(start..rows.len());
        }
    } else if !rows.is_empty() {
        components.push(0..rows.len());
    }
    result.preprocessing_ns = begin.elapsed().as_nanos();
    let model = if name == "C" {
        "C"
    } else if name == "B" {
        "B"
    } else {
        "A"
    };
    let solutions = if name == "parallel" && components.len() >= 8 {
        std::thread::scope(|scope| {
            let handles: Vec<_> = components
                .chunks(components.len().div_ceil(8))
                .map(|batch| {
                    let rows = &rows;
                    let normalized = &normalized;
                    scope.spawn(move || {
                        batch
                            .iter()
                            .map(|r| solve_component(jobs, &rows[r.clone()], normalized, model))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        })
    } else {
        components
            .iter()
            .map(|r| solve_component(jobs, &rows[r.clone()], &normalized, model))
            .collect()
    };
    for (range, solved) in components.iter().zip(solutions) {
        result.timeline_ns += solved.timeline_ns;
        result.solver_ns += solved.solver_ns;
        result.reconstruction_ns += solved.reconstruction_ns;
        result.vertices += solved.vertices;
        result.edges += solved.edges;
        result.augmentations += solved.augmentations;
        result.components += solved.components;
        for (&i, selected) in rows[range.clone()].iter().zip(solved.mask) {
            result.mask[i] = selected;
        }
    }
    result
}

pub fn verify(jobs: &[Job], profile: &[Segment], mask: &[bool]) -> i128 {
    assert_eq!(jobs.len(), mask.len());
    let rows: Vec<_> = (0..jobs.len())
        .filter(|&i| mask[i] && jobs[i].0 < jobs[i].1)
        .collect();
    assert!(
        feasible(jobs, &rows, &normalize(profile, false, jobs.len())),
        "infeasible mask"
    );
    jobs.iter()
        .zip(mask)
        .map(|(&(s, e, w), &selected)| {
            if s == e && w > 0 {
                assert!(selected);
            }
            if selected {
                assert!(w > 0);
                i128::from(w)
            } else {
                0
            }
        })
        .sum()
}

/// Exhaustive independent atomic-segment oracle; no graph or sweep reuse.
pub fn brute_force(jobs: &[Job], profile: &[Segment]) -> i128 {
    assert!(jobs.len() <= 12);
    let mut points: Vec<_> = jobs
        .iter()
        .flat_map(|r| [r.0, r.1])
        .chain(profile.iter().flat_map(|r| [r.0, r.1]))
        .collect();
    points.sort_unstable();
    points.dedup();
    (0usize..1usize << jobs.len())
        .filter_map(|bits| {
            for pair in points.windows(2) {
                let t = pair[0];
                let c = profile
                    .iter()
                    .find(|r| r.0 <= t && t < r.1)
                    .map_or(0, |r| r.2);
                let active = jobs
                    .iter()
                    .enumerate()
                    .filter(|&(i, &(s, e, _))| bits & (1 << i) != 0 && s <= t && t < e)
                    .count();
                if active > c {
                    return None;
                }
            }
            Some(
                jobs.iter()
                    .enumerate()
                    .filter(|&(i, _)| bits & (1 << i) != 0)
                    .map(|(_, r)| i128::from(r.2))
                    .sum(),
            )
        })
        .max()
        .unwrap()
}
