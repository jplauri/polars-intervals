//! Private conventional min-cost flow baseline, independently stored and solved.
use std::cmp::Reverse;
use std::collections::BinaryHeap;

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
    fn edge(&mut self, u: usize, v: usize, cap: usize, cost: i128) -> usize {
        let (a, b) = (self.graph[u].len(), self.graph[v].len());
        self.graph[u].push(Arc {
            to: v,
            reverse: b,
            residual: cap,
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

    fn send(&mut self, source: usize, sink: usize, mut amount: usize) {
        let n = self.graph.len();
        // Bellman-Ford initializes potentials for a conventional graph with
        // negative costs. It terminates early on this initially acyclic network.
        let mut h = vec![i128::MAX; n];
        h[source] = 0;
        for _ in 1..n {
            let mut changed = false;
            for u in 0..n {
                if h[u] == i128::MAX {
                    continue;
                }
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
        }
        while amount > 0 {
            let mut distance = vec![i128::MAX; n];
            let mut parent = vec![(usize::MAX, usize::MAX); n];
            let mut heap = BinaryHeap::new();
            distance[source] = 0;
            heap.push(Reverse((0, source)));
            while let Some(Reverse((d, u))) = heap.pop() {
                if d != distance[u] {
                    continue;
                }
                for (j, arc) in self.graph[u].iter().enumerate() {
                    if arc.residual == 0 {
                        continue;
                    }
                    let candidate = d + arc.cost + h[u] - h[arc.to];
                    if candidate < distance[arc.to] {
                        distance[arc.to] = candidate;
                        parent[arc.to] = (u, j);
                        heap.push(Reverse((candidate, arc.to)));
                    }
                }
            }
            assert_ne!(distance[sink], i128::MAX);
            for i in 0..n {
                if distance[i] != i128::MAX {
                    h[i] += distance[i];
                }
            }
            let mut push = amount;
            let mut v = sink;
            while v != source {
                let (u, j) = parent[v];
                push = push.min(self.graph[u][j].residual);
                v = u;
            }
            v = sink;
            while v != source {
                let (u, j) = parent[v];
                let reverse = self.graph[u][j].reverse;
                self.graph[u][j].residual -= push;
                self.graph[v][reverse].residual += push;
                v = u;
            }
            amount -= push;
        }
    }
}

pub fn solve(s: &[i64], e: &[i64], w: &[i64], k: usize) -> Vec<bool> {
    let mut mask: Vec<_> = (0..s.len()).map(|i| s[i] == e[i] && w[i] > 0).collect();
    if k == 0 {
        return mask;
    }
    let rows: Vec<_> = (0..s.len()).filter(|&i| s[i] < e[i] && w[i] > 0).collect();
    if rows.is_empty() {
        return mask;
    }
    let mut times: Vec<_> = rows.iter().flat_map(|&i| [s[i], e[i]]).collect();
    times.sort_unstable();
    times.dedup();
    let mut flow = Flow {
        graph: (0..times.len()).map(|_| Vec::new()).collect(),
    };
    for v in 1..times.len() {
        flow.edge(v - 1, v, k, 0);
    }
    let mut links = Vec::new();
    for &i in &rows {
        let u = times.binary_search(&s[i]).unwrap();
        let v = times.binary_search(&e[i]).unwrap();
        let j = flow.edge(u, v, 1, -i128::from(w[i]));
        links.push((i, u, j));
    }
    flow.send(0, times.len() - 1, k);
    for (i, u, j) in links {
        mask[i] = flow.graph[u][j].residual == 0;
    }
    mask
}

/// Feasibility checked independently by an event sweep (ends before starts).
pub fn verify(s: &[i64], e: &[i64], w: &[i64], k: usize, mask: &[bool]) -> i128 {
    assert_eq!(mask.len(), s.len());
    let mut objective = 0;
    let mut events = Vec::new();
    for i in 0..s.len() {
        assert!(s[i] <= e[i]);
        if s[i] == e[i] && w[i] > 0 {
            assert!(mask[i]);
        }
        if mask[i] {
            assert!(w[i] > 0);
            objective += i128::from(w[i]);
            if s[i] < e[i] {
                events.extend([(s[i], 1i64), (e[i], -1)]);
            }
        }
    }
    events.sort_unstable();
    let mut live = 0;
    for (_, delta) in events {
        live += delta;
        assert!(live >= 0 && live as usize <= k);
    }
    assert_eq!(live, 0);
    objective
}

/// Independent exhaustive oracle: count coverage at each selected start,
/// without sharing the flow formulation, event sweep or preprocessing.
pub fn brute_force(s: &[i64], e: &[i64], w: &[i64], k: usize) -> i128 {
    assert!(s.len() <= 24);
    (0usize..1 << s.len())
        .filter_map(|bits| {
            for &t in s {
                let live = (0..s.len())
                    .filter(|&i| bits & (1 << i) != 0 && s[i] <= t && t < e[i])
                    .count();
                if live > k {
                    return None;
                }
            }
            Some(
                (0..s.len())
                    .filter(|&i| bits & (1 << i) != 0)
                    .map(|i| i128::from(w[i]))
                    .sum(),
            )
        })
        .max()
        .unwrap()
}
