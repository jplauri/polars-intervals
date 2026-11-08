//! Exact minimum-cost flow shared by the capacity solvers.

use crate::IntervalError;
use crate::capacity::Row;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Segment<T> {
    pub start: T,
    pub end: T,
    pub capacity: usize,
}

pub(crate) struct Timeline<T> {
    pub endpoints: Vec<T>,
    pub capacities: Vec<usize>,
    pub jobs: Vec<(usize, usize)>,
}

impl<T: Ord + Copy> Timeline<T> {
    pub(crate) fn new(rows: &[Row<T>], profile: &[Segment<T>]) -> Self {
        if rows.is_empty() {
            return Self {
                endpoints: Vec::new(),
                capacities: Vec::new(),
                jobs: Vec::new(),
            };
        }
        let start = rows.iter().map(|r| r.start).min().unwrap();
        let end = rows.iter().map(|r| r.end).max().unwrap();
        let begin = profile.partition_point(|segment| segment.end <= start);
        let finish = profile.partition_point(|segment| segment.start < end);
        let relevant = &profile[begin..finish];
        let mut endpoints = Vec::with_capacity(2 * (rows.len() + relevant.len()));
        for row in rows {
            endpoints.extend([row.start, row.end]);
        }
        for segment in relevant {
            endpoints.extend([segment.start.max(start), segment.end.min(end)]);
        }
        endpoints.sort_unstable();
        endpoints.dedup();
        let mut capacities = Vec::with_capacity(endpoints.len() - 1);
        let mut i = 0;
        for &point in &endpoints[..endpoints.len() - 1] {
            while i < relevant.len() && relevant[i].end <= point {
                i += 1;
            }
            capacities.push(if i < relevant.len() && relevant[i].start <= point {
                relevant[i].capacity.min(rows.len())
            } else {
                0
            });
        }
        let jobs = rows
            .iter()
            .map(|row| {
                (
                    endpoints.binary_search(&row.start).unwrap(),
                    endpoints.binary_search(&row.end).unwrap(),
                )
            })
            .collect();
        Self {
            endpoints,
            capacities,
            jobs,
        }
    }
}

pub(crate) struct Edge {
    pub to: usize,
    pub capacity: usize,
    pub cost: i128,
}

/// Capacity-change transshipment, represented as source/sink min-cost flow.
///
/// Put supply delta_j = c_j - c_(j-1) at each timeline node, interpreting the
/// capacity before/after the horizon as zero. Positive deltas receive a source
/// arc; negative deltas receive a sink arc. Timeline arcs have capacity c_j,
/// and each forward job arc has unit capacity and cost -weight.
///
/// CUT PROOF: sum flow conservation over timeline nodes 0..=j. The net supply
/// telescopes to c_j. All original arcs go forwards, so exactly c_j units cross
/// this cut: timeline flow plus chosen jobs spanning segment j. Nonnegative
/// timeline flow therefore implies selected concurrency <= c_j. Conversely,
/// for any feasible selected set put c_j - concurrency_j on timeline arc j;
/// those nonnegative flows satisfy all node supplies and bounds. Thus feasible
/// subsets and feasible integral flows correspond, with cost = -objective.
/// Integral capacities and bottleneck augmentation preserve integrality.
///
/// CIRCULATION MODEL: reverse every timeline arc, with capacity c_j. A chosen
/// forward job must return one unit across every backward cut it spans; summing
/// conservation over a prefix proves backward flow equals selected concurrency.
/// Start by saturating every negative-cost job arc. The residual network then
/// has only zero-cost backward timeline arcs and positive-cost job rejection
/// arcs. Restore conservation by sending excess at ends to deficits at starts.
/// Rejecting all jobs proves feasibility. SSP minimizes rejection weight, hence
/// maximizes retained weight; the omitted initial saturated cost is constant.
/// This avoids negative-cycle cancellation and shares the same exact SSP.
pub(crate) struct Network {
    // Paired residual edges: the reverse of edge e is e ^ 1. CSR adjacency
    // stores edge indices, keeping both edge and adjacency buffers contiguous.
    pub edges: Vec<Edge>,
    pub offsets: Vec<usize>,
    pub adjacent: Vec<usize>,
    // Job edge pairs follow timeline pairs, in row order.
    pub interval_start: usize,
    pub job_count: usize,
    pub required: usize,
    pub circulation: bool,
    one_segment: bool,
}

impl Network {
    /// Constant capacity as one profile segment spanning `rows`.
    ///
    /// Callers solve only when `capacity` is below peak concurrency. Tightening
    /// then cannot reduce the supply, and concurrency is at most the circulation
    /// supply, so the profile solver's adaptive choice is always this network.
    pub(crate) fn one_segment<T: Ord + Copy>(rows: &[Row<T>], capacity: usize) -> Self {
        let segment = Segment {
            start: rows[0].start,
            end: rows.iter().map(|row| row.end).max().unwrap(),
            capacity,
        };
        let mut timeline = Timeline::new(rows, &[segment]);
        // Endpoint values are done. Free them before the largest allocations.
        timeline.endpoints = Vec::new();
        let mut network = Self::build(&timeline, rows, &timeline.capacities, None);
        network.one_segment = true;
        network
    }

    pub(crate) fn build<T>(
        timeline: &Timeline<T>,
        rows: &[Row<T>],
        capacities: &[usize],
        balances: Option<Vec<i128>>,
    ) -> Self {
        // Timeline breakpoints, plus the source and sink.
        let vertices = capacities.len() + 3;
        let circulation = balances.is_some();
        let supplies = match &balances {
            Some(balances) => balances.len(),
            None => 2 + capacities.windows(2).filter(|c| c[0] != c[1]).count(),
        };
        let mut network = Self {
            edges: Vec::with_capacity(2 * (capacities.len() + rows.len() + supplies)),
            offsets: vec![0; vertices + 1],
            adjacent: Vec::new(),
            interval_start: 2 * timeline.capacities.len(),
            job_count: rows.len(),
            required: 0,
            circulation,
            one_segment: false,
        };
        for (j, &capacity) in capacities.iter().enumerate() {
            if circulation {
                network.add_edge(j + 2, j + 1, capacity, 0);
            } else {
                network.add_edge(j + 1, j + 2, capacity, 0);
            }
        }
        for (&(start, end), row) in timeline.jobs.iter().zip(rows) {
            if circulation {
                network.add_edge(end + 1, start + 1, 1, row.weight);
            } else {
                network.add_edge(start + 1, end + 1, 1, -row.weight);
            }
        }
        if let Some(balances) = balances {
            for (j, balance) in balances.into_iter().enumerate() {
                if balance > 0 {
                    network.add_edge(0, j + 1, balance as usize, 0);
                    network.required += balance as usize;
                } else if balance < 0 {
                    network.add_edge(j + 1, vertices - 1, (-balance) as usize, 0);
                }
            }
        } else {
            let mut previous = 0;
            for (j, capacity) in capacities.iter().copied().chain([0]).enumerate() {
                if capacity > previous {
                    network.add_edge(0, j + 1, capacity - previous, 0);
                    network.required += capacity - previous;
                } else if capacity < previous {
                    network.add_edge(j + 1, vertices - 1, previous - capacity, 0);
                }
                previous = capacity;
            }
        }
        for v in 1..=vertices {
            network.offsets[v] += network.offsets[v - 1];
        }
        network.adjacent.resize(network.edges.len(), 0);
        let mut cursor = network.offsets[..vertices].to_vec();
        for e in 0..network.edges.len() {
            let from = network.edges[e ^ 1].to;
            network.adjacent[cursor[from]] = e;
            cursor[from] += 1;
        }
        network
    }

    fn add_edge(&mut self, from: usize, to: usize, capacity: usize, cost: i128) {
        self.edges.push(Edge { to, capacity, cost });
        self.edges.push(Edge {
            to: from,
            capacity: 0,
            cost: -cost,
        });
        self.offsets[from + 1] += 1;
        self.offsets[to + 1] += 1;
    }

    pub(crate) fn vertices(&self) -> usize {
        self.offsets.len() - 1
    }

    pub(crate) fn solve(&mut self) -> Result<usize, IntervalError> {
        let n = self.vertices();
        let sink = n - 1;
        let mut potential = vec![0i128; n];
        let mut previous = vec![usize::MAX; n];
        // A initially has only forward live edges. One DAG pass from a virtual
        // source to ALL vertices gives feasible potentials even across zero
        // capacity stretches. C starts with nonnegative costs, so the same pass
        // simply leaves its initially valid zero potentials unchanged.
        for v in 0..n {
            for &e in &self.adjacent[self.offsets[v]..self.offsets[v + 1]] {
                let edge = &self.edges[e];
                if edge.capacity > 0 {
                    let next = add(potential[v], edge.cost)?;
                    if next < potential[edge.to]
                        || (next == potential[edge.to] && previous[edge.to] == usize::MAX)
                    {
                        potential[edge.to] = next;
                        previous[edge.to] = e;
                    }
                }
            }
        }
        let mut distance = vec![u128::MAX; n];
        let mut queue = BinaryHeap::new();
        let (mut sent, mut augmentations) = (0, 0);
        // One segment leaves every timeline edge residual until the last unit,
        // so the source reaches all vertices and the DAG pass already holds the
        // first shortest path. Other networks need a real first search.
        let mut search = true;
        while sent < self.required {
            if search {
                distance.fill(u128::MAX);
                previous.fill(usize::MAX);
                distance[0] = 0;
                queue.push(Reverse((0u128, 0usize)));
                // Strict relaxation plus (distance, vertex) ordering resolves ties
                // deterministically and prevents predecessor cycles on zero costs.
                while let Some(Reverse((d, v))) = queue.pop() {
                    if d != distance[v] {
                        continue;
                    }
                    for &e in &self.adjacent[self.offsets[v]..self.offsets[v + 1]] {
                        let edge = &self.edges[e];
                        if edge.capacity == 0 {
                            continue;
                        }
                        let reduced = reduced_cost(potential[v], potential[edge.to], edge.cost);
                        // Unsigned distances also represent non-shortest detours
                        // above i128::MAX without overflowing a signed accumulator.
                        let next = d.saturating_add(reduced);
                        if next < distance[edge.to] {
                            distance[edge.to] = next;
                            previous[edge.to] = e;
                            queue.push(Reverse((next, edge.to)));
                        }
                    }
                }
                // A's all-unused flow, or C's rejection of all jobs, proves that
                // every requested corrective flow exists.
                assert_ne!(previous[sink], usize::MAX, "transshipment must be feasible");
                let cap = distance[sink];
                for (p, &d) in potential.iter_mut().zip(&distance) {
                    // Source reachability only shrinks: new reverse arcs join
                    // vertices already reachable along the augmentation. Freeze
                    // unreachable labels to avoid accumulating irrelevant offsets.
                    // Reduced costs stay nonnegative throughout the reachable set.
                    if d != u128::MAX {
                        *p = p
                            .checked_add_unsigned(d.min(cap))
                            .ok_or(IntervalError::WeightOverflow)?;
                    }
                }
            }
            search = true;
            let mut amount = self.required - sent;
            let mut v = sink;
            while v != 0 {
                let e = previous[v];
                amount = amount.min(self.edges[e].capacity);
                v = self.edges[e ^ 1].to;
            }
            v = sink;
            while v != 0 {
                let e = previous[v];
                self.edges[e].capacity -= amount;
                self.edges[e ^ 1].capacity += amount;
                v = self.edges[e ^ 1].to;
            }
            sent += amount;
            augmentations += 1;
        }
        Ok(augmentations)
    }

    /// Solve, then report the original index of each selected row.
    pub(crate) fn select<T>(
        mut self,
        rows: &[Row<T>],
        emit: &mut dyn FnMut(usize),
    ) -> Result<(), IntervalError> {
        self.solve()?;
        for (i, row) in rows.iter().enumerate() {
            if self.selected(i) {
                emit(row.index);
            }
        }
        Ok(())
    }

    pub(crate) fn reconstruct<T>(&self, rows: &[Row<T>], mask: &mut [bool]) {
        debug_assert_eq!(rows.len(), self.job_count);
        for (i, row) in rows.iter().enumerate() {
            mask[row.index] = self.selected(i);
        }
    }

    pub(crate) fn selected(&self, row: usize) -> bool {
        let e = self.interval_start + 2 * row;
        if self.circulation {
            self.edges[e].capacity == 1
        } else {
            self.edges[e ^ 1].capacity == 1
        }
    }
}

pub(crate) fn add(a: i128, b: i128) -> Result<i128, IntervalError> {
    a.checked_add(b).ok_or(IntervalError::WeightOverflow)
}

pub(crate) fn reduced_cost(from: i128, to: i128, cost: i128) -> u128 {
    // Nearly every residual edge fits i128, so this path has no sign branches.
    if let Some(reduced) = from.checked_sub(to).and_then(|gap| gap.checked_add(cost)) {
        assert!(reduced >= 0, "negative reduced cost");
        return reduced as u128;
    }
    // A non-shortest residual detour may exceed i128::MAX even when the
    // optimum fits. Subtract labels by unsigned magnitude before adding cost.
    // Values beyond u128::MAX can safely saturate just like Dijkstra distances.
    if from >= to {
        let difference = from.abs_diff(to);
        if cost >= 0 {
            difference.saturating_add(cost as u128)
        } else {
            difference
                .checked_sub(cost.unsigned_abs())
                .expect("negative reduced cost")
        }
    } else {
        assert!(cost >= 0, "negative reduced cost");
        (cost as u128)
            .checked_sub(from.abs_diff(to))
            .expect("negative reduced cost")
    }
}

/// Solve independent components on up to `workers` scoped threads.
///
/// `solve` reports the original index of each selected row. Each worker owns a
/// contiguous batch and handles join in order, so completion order is irrelevant.
pub(crate) fn solve_components<T: Sync>(
    rows: &[Row<T>],
    ranges: &[Range<usize>],
    mask: &mut [bool],
    workers: usize,
    solve: impl Fn(&[Row<T>], &mut dyn FnMut(usize)) -> Result<(), IntervalError> + Sync,
) -> Result<(), IntervalError> {
    if workers <= 1 {
        for range in ranges {
            solve(&rows[range.clone()], &mut |index| mask[index] = true)?;
        }
        return Ok(());
    }
    std::thread::scope(|scope| {
        let solve = &solve;
        let handles: Vec<_> = ranges
            .chunks(ranges.len().div_ceil(workers))
            .map(|batch| {
                scope.spawn(move || {
                    let mut selected = Vec::new();
                    for range in batch {
                        solve(&rows[range.clone()], &mut |index| selected.push(index))?;
                    }
                    Ok::<_, IntervalError>(selected)
                })
            })
            .collect();
        for handle in handles {
            for index in handle.join().expect("capacity worker panicked")? {
                mask[index] = true;
            }
        }
        Ok(())
    })
}
