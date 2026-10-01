use crate::capacity::{Prepared, Row, check_objective, components, prepare};
use crate::{IntervalError, check_len, max_weight_with_capacity};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Select a globally maximum-weight subset of fixed intervals subject to a
/// piecewise-constant capacity profile.
///
/// Both jobs and profile segments are half-open. Profile rows may be unsorted,
/// but nonempty rows must not overlap. Gaps and the region outside the profile
/// have capacity zero. Empty profile rows are validated, then ignored. Capacities
/// must be nonnegative integers; values above the number of jobs are safely
/// clamped because every selected nonempty job consumes one unit.
///
/// Returns a Boolean mask in original job order. Positive empty jobs are always
/// selected and consume no capacity. Nonpositive jobs are omitted. Ties are
/// deterministic for identical inputs; the particular optimal mask is not a
/// stable contract. Weights and the objective use checked `i128` arithmetic.
///
/// Constant profiles dispatch to [`max_weight_with_capacity`], including its
/// specialized capacity-one path. Otherwise the implementation normalizes the
/// profile, drops jobs crossing capacity zero, detects when all candidates fit,
/// and solves independent overlap components by exact min-cost flow. An adaptive
/// choice uses capacity-change transshipment or backward-timeline circulation
/// with initially saturated job arcs. Both share one residual-flow kernel.
/// Timeline size depends on breakpoints, never elapsed time.
/// With `v` breakpoints, `e = O(n + v)` edges and `a` bottleneck augmentations,
/// a constrained component takes `O(a * e * log(e + 1))` time and `O(n + v)`
/// space after sorting. Augmentations send the full residual path bottleneck;
/// Exact candidate-derived capacity bounds guarantee `a <= n`, independent of raw
/// capacity magnitude and capacity-profile variation.
///
/// # Errors
///
/// Rejects mismatched slice lengths, reversed job/profile intervals, overlapping
/// nonempty profile rows, negative capacities, and checked weight overflow.
/// Validation also covers empty profiles/jobs and nonpositive-weight jobs.
///
/// # Examples
///
/// ```
/// use intervals_core::max_weight_with_capacity_profile;
/// let selected = max_weight_with_capacity_profile(
///     &[0, 0, 5], &[10, 5, 10], &[15, 10, 10],
///     &[0, 5], &[5, 10], &[1, 2],
/// )?;
/// assert_eq!(selected, [true, false, true]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn max_weight_with_capacity_profile<T, W, C>(
    starts: &[T],
    ends: &[T],
    weights: &[W],
    profile_starts: &[T],
    profile_ends: &[T],
    capacities: &[C],
) -> Result<Vec<bool>, IntervalError>
where
    T: Ord + Copy + Sync,
    W: Copy,
    C: Copy,
    i128: From<W> + From<C>,
{
    let profile = normalize(profile_starts, profile_ends, capacities, starts.len())?;
    // Gap insertion and coalescing make an all-zero profile empty or one row.
    // Avoid sorting jobs even when their horizon extends outside this profile.
    if profile.is_empty() || (profile.len() == 1 && profile[0].capacity == 0) {
        return max_weight_with_capacity(starts, ends, weights, 0);
    }
    // Detect constant capacity before sorting jobs, so dispatch adds only the
    // profile normalization and a linear horizon scan to the existing solver.
    // That solver still validates every original job and every slice length.
    let horizon = starts
        .iter()
        .zip(ends)
        .zip(weights)
        .filter(|((start, end), weight)| start < end && i128::from(**weight) > 0)
        .fold(None, |horizon: Option<(T, T)>, ((&start, &end), _)| {
            Some(horizon.map_or((start, end), |(a, b)| (a.min(start), b.max(end))))
        });
    if let Some((start, end)) = horizon
        && let Some(capacity) = constant_capacity(&profile, start, end)
    {
        return max_weight_with_capacity(starts, ends, weights, capacity);
    }
    let Prepared { mut rows, mut mask } = prepare(starts, ends, weights, usize::MAX)?;
    if !rows.is_empty() {
        let mut timeline = Timeline::new(&rows, &profile);
        filter_impossible(&mut rows, &mut timeline);
        if all_feasible(&timeline) {
            for row in &rows {
                mask[row.index] = true;
            }
        } else {
            let ranges = components(&rows);
            if ranges.len() == 1 {
                let mut network = Network::new(&timeline, &rows);
                network.solve()?;
                network.reconstruct(&rows, &mut mask);
            } else {
                // Global scratch is no longer needed. Release it before
                // allocating independent component networks and worker buffers.
                drop(timeline);
                let workers = if rows.len() >= 16_384 && ranges.len() >= 8 {
                    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
                } else {
                    1
                };
                solve_components(&rows, &profile, &ranges, &mut mask, workers)?;
            }
        }
    }
    check_objective(weights, &mask)?;
    Ok(mask)
}

fn solve_component<T: Ord + Copy>(
    rows: &[Row<T>],
    profile: &[Segment<T>],
) -> Result<Vec<usize>, IntervalError> {
    let timeline = Timeline::new(rows, profile);
    if all_feasible(&timeline) {
        return Ok(rows.iter().map(|row| row.index).collect());
    }
    let mut network = Network::new(&timeline, rows);
    network.solve()?;
    Ok(rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| network.selected(i).then_some(row.index))
        .collect())
}

fn solve_components<T: Ord + Copy + Sync>(
    rows: &[Row<T>],
    profile: &[Segment<T>],
    ranges: &[std::ops::Range<usize>],
    mask: &mut [bool],
    workers: usize,
) -> Result<(), IntervalError> {
    if workers <= 1 {
        for range in ranges {
            for index in solve_component(&rows[range.clone()], profile)? {
                mask[index] = true;
            }
        }
        return Ok(());
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = ranges
            .chunks(ranges.len().div_ceil(workers))
            .map(|batch| {
                scope.spawn(move || {
                    let mut selected = Vec::new();
                    for range in batch {
                        selected.extend(solve_component(&rows[range.clone()], profile)?);
                    }
                    Ok::<_, IntervalError>(selected)
                })
            })
            .collect();
        // Join and restore original row IDs in deterministic component order.
        for handle in handles {
            for index in handle.join().expect("capacity-profile worker panicked")? {
                mask[index] = true;
            }
        }
        Ok(())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Segment<T> {
    start: T,
    end: T,
    capacity: usize,
}

/// Sort packed rows, reject ambiguous overlaps, insert zero gaps, and coalesce.
/// Row IDs remain attached until overlap validation to report original indices.
fn normalize<T: Ord + Copy, C: Copy>(
    starts: &[T],
    ends: &[T],
    capacities: &[C],
    limit: usize,
) -> Result<Vec<Segment<T>>, IntervalError>
where
    i128: From<C>,
{
    check_len(
        ("profile starts", starts.len()),
        ("profile ends", ends.len()),
    )?;
    check_len(
        ("profile segments", starts.len()),
        ("capacities", capacities.len()),
    )?;
    let mut packed = Vec::with_capacity(starts.len());
    for (index, ((&start, &end), &capacity)) in starts.iter().zip(ends).zip(capacities).enumerate()
    {
        if start > end {
            return Err(IntervalError::InvalidProfileInterval { index });
        }
        let capacity = i128::from(capacity);
        if capacity < 0 {
            return Err(IntervalError::NegativeCapacity { index });
        }
        if start < end {
            packed.push((start, end, capacity.min(limit as i128) as usize, index));
        }
    }
    packed.sort_unstable_by_key(|&(start, end, _, index)| (start, end, index));
    let mut result: Vec<Segment<T>> = Vec::with_capacity(packed.len());
    let mut previous: Option<(T, usize)> = None;
    for (start, end, capacity, index) in packed {
        if let Some((previous_end, previous_index)) = previous {
            if start < previous_end {
                return Err(IntervalError::OverlappingProfile {
                    first: previous_index,
                    second: index,
                });
            }
            if previous_end < start {
                push_segment(&mut result, previous_end, start, 0);
            }
        }
        push_segment(&mut result, start, end, capacity);
        previous = Some((end, index));
    }
    Ok(result)
}

fn push_segment<T: Copy + Eq>(result: &mut Vec<Segment<T>>, start: T, end: T, capacity: usize) {
    if let Some(previous) = result.last_mut()
        && previous.end == start
        && previous.capacity == capacity
    {
        previous.end = end;
    } else {
        result.push(Segment {
            start,
            end,
            capacity,
        });
    }
}

fn constant_capacity<T: Ord + Copy>(profile: &[Segment<T>], start: T, end: T) -> Option<usize> {
    let i = profile.partition_point(|segment| segment.end <= start);
    let Some(segment) = profile.get(i) else {
        return Some(0);
    };
    if end <= segment.start {
        Some(0)
    } else if segment.start <= start && end <= segment.end {
        Some(segment.capacity)
    } else {
        None
    }
}

struct Timeline<T> {
    endpoints: Vec<T>,
    capacities: Vec<usize>,
    jobs: Vec<(usize, usize)>,
}

impl<T: Ord + Copy> Timeline<T> {
    fn new(rows: &[Row<T>], profile: &[Segment<T>]) -> Self {
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

fn filter_impossible<T>(rows: &mut Vec<Row<T>>, timeline: &mut Timeline<T>) {
    let mut zeros = Vec::with_capacity(timeline.capacities.len() + 1);
    zeros.push(0usize);
    for &capacity in &timeline.capacities {
        zeros.push(zeros.last().unwrap() + usize::from(capacity == 0));
    }
    let mut i = 0;
    rows.retain(|_| {
        let (start, end) = timeline.jobs[i];
        i += 1;
        zeros[start] == zeros[end]
    });
    timeline
        .jobs
        .retain(|&(start, end)| zeros[start] == zeros[end]);
}

fn all_feasible<T>(timeline: &Timeline<T>) -> bool {
    let mut changes = vec![0i128; timeline.endpoints.len()];
    for &(start, end) in &timeline.jobs {
        changes[start] += 1;
        changes[end] -= 1;
    }
    let mut active = 0;
    for (&change, &capacity) in changes.iter().zip(&timeline.capacities) {
        active += change;
        if active > capacity as i128 {
            return false;
        }
    }
    true
}

struct Edge {
    to: usize,
    capacity: usize,
    cost: i128,
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
struct Network {
    edges: Vec<Edge>,
    offsets: Vec<usize>,
    adjacent: Vec<usize>,
    interval_start: usize,
    job_count: usize,
    required: usize,
    circulation: bool,
}

impl Network {
    fn new<T>(timeline: &Timeline<T>, rows: &[Row<T>]) -> Self {
        let mut counts = vec![(0usize, 0usize); timeline.endpoints.len()];
        for &(start, end) in &timeline.jobs {
            counts[start].0 += 1;
            counts[end].1 += 1;
        }
        let tightened = tighten_capacities(&timeline.capacities, &counts);
        let original_supply = capacity_supply(&timeline.capacities);
        let tightened_supply = capacity_supply(&tightened);
        // Tightening can introduce extra rises on otherwise flat sparse
        // timelines. Keep whichever representation has less total supply.
        let (capacities, supply) = if tightened_supply < original_supply {
            (tightened.as_slice(), tightened_supply)
        } else {
            (timeline.capacities.as_slice(), original_supply)
        };
        let circulation_supply: u128 = counts
            .iter()
            .map(|&(starts, ends)| ends.saturating_sub(starts) as u128)
            .sum();
        // Controlled release measurements show circulation wins by ~10-90x
        // when capacity changes create many supplies but job endpoints cancel.
        // Prefer A within a factor four, avoiding C's expensive rejection
        // augmentations for dense jobs. Tightening bounds F_A <= n, while
        // F_C <= n by construction, so either model needs at most n augmentations.
        if supply > 4 * circulation_supply {
            let balances = counts
                .into_iter()
                .map(|(starts, ends)| ends as i128 - starts as i128)
                .collect();
            Self::build(timeline, rows, capacities, Some(balances))
        } else {
            Self::build(timeline, rows, capacities, None)
        }
    }

    // Forced model construction lets tests exercise both formulations.
    #[cfg(test)]
    fn new_transshipment<T>(timeline: &Timeline<T>, rows: &[Row<T>]) -> Self {
        Self::build(timeline, rows, &timeline.capacities, None)
    }

    #[cfg(test)]
    fn new_circulation<T>(timeline: &Timeline<T>, rows: &[Row<T>]) -> Self {
        let mut balances = vec![0; timeline.endpoints.len()];
        for &(start, end) in &timeline.jobs {
            balances[start] -= 1;
            balances[end] += 1;
        }
        Self::build(timeline, rows, &timeline.capacities, Some(balances))
    }

    fn build<T>(
        timeline: &Timeline<T>,
        rows: &[Row<T>],
        capacities: &[usize],
        balances: Option<Vec<i128>>,
    ) -> Self {
        let vertices = timeline.endpoints.len() + 2;
        let circulation = balances.is_some();
        let mut network = Self {
            edges: Vec::with_capacity(2 * (rows.len() + 2 * vertices)),
            offsets: vec![0; vertices + 1],
            adjacent: Vec::new(),
            interval_start: 2 * timeline.capacities.len(),
            job_count: rows.len(),
            required: 0,
            circulation,
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

    fn vertices(&self) -> usize {
        self.offsets.len() - 1
    }

    fn solve(&mut self) -> Result<usize, IntervalError> {
        let n = self.vertices();
        let sink = n - 1;
        let mut potential = vec![0i128; n];
        // A initially has only forward live edges. One DAG pass from a virtual
        // source to ALL vertices gives feasible potentials even across zero
        // capacity stretches. C starts with nonnegative costs, so the same pass
        // simply leaves its initially valid zero potentials unchanged.
        for v in 0..n {
            for &e in &self.adjacent[self.offsets[v]..self.offsets[v + 1]] {
                let edge = &self.edges[e];
                if edge.capacity > 0 {
                    let next = add(potential[v], edge.cost)?;
                    potential[edge.to] = potential[edge.to].min(next);
                }
            }
        }
        let mut distance = vec![u128::MAX; n];
        let mut previous = vec![usize::MAX; n];
        let mut queue = BinaryHeap::new();
        let (mut sent, mut augmentations) = (0, 0);
        while sent < self.required {
            distance.fill(u128::MAX);
            previous.fill(usize::MAX);
            distance[0] = 0;
            queue.push(Reverse((0u128, 0usize)));
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

    fn reconstruct<T>(&self, rows: &[Row<T>], mask: &mut [bool]) {
        debug_assert_eq!(rows.len(), self.job_count);
        for (i, row) in rows.iter().enumerate() {
            mask[row.index] = self.selected(i);
        }
    }

    fn selected(&self, row: usize) -> bool {
        let e = self.interval_start + 2 * row;
        if self.circulation {
            self.edges[e].capacity == 1
        } else {
            self.edges[e ^ 1].capacity == 1
        }
    }
}

fn add(a: i128, b: i128) -> Result<i128, IntervalError> {
    a.checked_add(b).ok_or(IntervalError::WeightOverflow)
}

fn capacity_supply(capacities: &[usize]) -> u128 {
    let mut previous = 0;
    capacities.iter().fold(0u128, |supply, &capacity| {
        let rise = capacity.saturating_sub(previous);
        previous = capacity;
        supply.saturating_add(rise as u128)
    })
}

/// Every feasible selected concurrency x satisfies x_j <= x_(j-1)+starts_j
/// and x_j <= x_(j+1)+ends_(j+1), counting ALL candidate starts/ends. Induction
/// proves these sweeps retain every feasible subset while capacities only
/// decrease. Backward clipping preserves the forward bound: if predecessor j-1
/// is lowered to new_c_j+ends_j, then new_c_j <= new_c_(j-1)+starts_j trivially;
/// otherwise the original forward inequality still applies. Thus total positive
/// capacity rises are at most total candidate starts, n. Adding removed capacity
/// slack to unused flow restores the original-profile cut invariant exactly.
fn tighten_capacities(capacities: &[usize], counts: &[(usize, usize)]) -> Vec<usize> {
    let mut tightened = capacities.to_vec();
    let mut previous = 0usize;
    for (j, capacity) in tightened.iter_mut().enumerate() {
        *capacity = (*capacity).min(previous.saturating_add(counts[j].0));
        previous = *capacity;
    }
    let mut next = 0usize;
    for (j, capacity) in tightened.iter_mut().enumerate().rev() {
        *capacity = (*capacity).min(next.saturating_add(counts[j + 1].1));
        next = *capacity;
    }
    tightened
}

fn reduced_cost(from: i128, to: i128, cost: i128) -> u128 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn segments(capacities: &[u8]) -> Vec<Segment<i32>> {
        let starts: Vec<_> = (0..capacities.len()).map(|i| i as i32 - 6).collect();
        let ends: Vec<_> = starts.iter().map(|s| s + 1).collect();
        normalize(&starts, &ends, capacities, usize::MAX).unwrap()
    }

    fn flow_invariants(rows: &[(i32, i32, i64)], capacities: &[u8]) {
        let starts: Vec<_> = rows.iter().map(|r| r.0).collect();
        let ends: Vec<_> = rows.iter().map(|r| r.1).collect();
        let weights: Vec<_> = rows.iter().map(|r| r.2).collect();
        let prepared = prepare(&starts, &ends, &weights, usize::MAX).unwrap();
        if prepared.rows.is_empty() {
            return;
        }
        let timeline = Timeline::new(&prepared.rows, &segments(capacities));
        let a = network_invariants(
            Network::new_transshipment(&timeline, &prepared.rows),
            &prepared,
            &timeline,
        );
        let c = network_invariants(
            Network::new_circulation(&timeline, &prepared.rows),
            &prepared,
            &timeline,
        );
        assert_eq!(a, c);
        let mut adaptive = Network::new(&timeline, &prepared.rows);
        assert!(adaptive.solve().unwrap() <= prepared.rows.len());
        let mut adaptive_mask = prepared.mask.clone();
        adaptive.reconstruct(&prepared.rows, &mut adaptive_mask);
        assert_eq!(
            prepared
                .rows
                .iter()
                .filter(|r| adaptive_mask[r.index])
                .map(|r| r.weight)
                .sum::<i128>(),
            a
        );
        for (j, &raw_capacity) in timeline.capacities.iter().enumerate() {
            let effective_capacity =
                adaptive.edges[2 * j].capacity + adaptive.edges[2 * j + 1].capacity;
            let active = timeline
                .jobs
                .iter()
                .zip(&prepared.rows)
                .filter(|((start, end), row)| *start <= j && j < *end && adaptive_mask[row.index])
                .count();
            let unused = if adaptive.circulation {
                effective_capacity - adaptive.edges[2 * j + 1].capacity
            } else {
                adaptive.edges[2 * j + 1].capacity
            };
            assert_eq!(
                unused + active + (raw_capacity - effective_capacity),
                raw_capacity
            );
        }
    }

    fn network_invariants(
        mut network: Network,
        prepared: &Prepared<i32>,
        timeline: &Timeline<i32>,
    ) -> i128 {
        let original: Vec<_> = network.edges.iter().map(|e| e.capacity).collect();
        network.solve().unwrap();
        let mut mask = prepared.mask.clone();
        network.reconstruct(&prepared.rows, &mut mask);
        let mut balance = vec![0i128; network.vertices()];
        let mut cost = 0;
        for e in (0..network.edges.len()).step_by(2) {
            let forward = &network.edges[e];
            let reverse = &network.edges[e ^ 1];
            assert_eq!(forward.capacity + reverse.capacity, original[e]);
            assert_eq!(forward.cost, -reverse.cost);
            let flow = reverse.capacity as i128;
            balance[reverse.to] -= flow;
            balance[forward.to] += flow;
            cost += forward.cost * flow;
        }
        assert_eq!(balance[0], -(network.required as i128));
        assert_eq!(*balance.last().unwrap(), network.required as i128);
        assert!(balance[1..balance.len() - 1].iter().all(|&b| b == 0));
        // Positive empty weights never enter the network objective.
        let selected_nonempty: i128 = prepared
            .rows
            .iter()
            .filter(|r| mask[r.index])
            .map(|r| r.weight)
            .sum();
        if network.circulation {
            let initial_cost: i128 = -prepared.rows.iter().map(|r| r.weight).sum::<i128>();
            assert_eq!(initial_cost + cost, -selected_nonempty);
        } else {
            assert_eq!(cost, -selected_nonempty);
        }
        for (i, row) in prepared.rows.iter().enumerate() {
            let flow = network.edges[network.interval_start + 2 * i + 1].capacity;
            assert!(flow <= 1);
            assert_eq!(mask[row.index], flow == usize::from(!network.circulation));
        }
        let k = *timeline.capacities.iter().max().unwrap();
        for (j, &capacity) in timeline.capacities.iter().enumerate() {
            let timeline_flow = network.edges[2 * j + 1].capacity;
            let active = timeline
                .jobs
                .iter()
                .zip(&prepared.rows)
                .filter(|((start, end), row)| *start <= j && j < *end && mask[row.index])
                .count();
            let unused = if network.circulation {
                assert_eq!(timeline_flow, active, "backward circulation cut {j}");
                assert!(timeline_flow <= capacity);
                capacity - timeline_flow
            } else {
                assert_eq!(timeline_flow + active, capacity, "atomic cut {j}");
                timeline_flow
            };
            assert!(unused <= capacity);
            // B: undo the lower-bound substitution; each cut then carries K.
            let lower = k - capacity;
            let fixed_k_timeline = lower + unused;
            assert!((lower..=k).contains(&fixed_k_timeline));
            assert_eq!(fixed_k_timeline + active, k);
            // C: its backward arc carries exactly selected concurrency.
            assert!(active <= capacity);
        }
        // Independent Bellman-Ford optimality certificate: there is no
        // improving residual negative cycle, even in unreachable components.
        let mut distance = vec![0i128; network.vertices()];
        for iteration in 0..network.vertices() {
            let mut changed = false;
            for (e, edge) in network.edges.iter().enumerate() {
                if edge.capacity > 0 {
                    let from = network.edges[e ^ 1].to;
                    let candidate = distance[from] + edge.cost;
                    if candidate < distance[edge.to] {
                        distance[edge.to] = candidate;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            assert!(
                iteration + 1 < network.vertices(),
                "negative residual cycle"
            );
        }
        selected_nonempty
    }

    #[test]
    fn normalization_validates_sorts_gaps_coalesces_and_ignores_empties() {
        assert_eq!(normalize::<i32, i32>(&[], &[], &[], 10).unwrap(), []);
        assert_eq!(
            normalize(&[8, 0, 2, 5], &[10, 2, 5, 5], &[3, 2, 2, 100], 2).unwrap(),
            [
                Segment {
                    start: 0,
                    end: 5,
                    capacity: 2
                },
                Segment {
                    start: 5,
                    end: 8,
                    capacity: 0
                },
                Segment {
                    start: 8,
                    end: 10,
                    capacity: 2
                },
            ]
        );
        assert_eq!(
            normalize(&[0, 5], &[7, 10], &[0, 0], 0),
            Err(IntervalError::OverlappingProfile {
                first: 0,
                second: 1
            })
        );
        assert_eq!(
            normalize(&[5], &[5], &[-1], 10),
            Err(IntervalError::NegativeCapacity { index: 0 })
        );
        assert_eq!(
            normalize(&[5], &[4], &[0], 10),
            Err(IntervalError::InvalidProfileInterval { index: 0 })
        );
    }

    #[test]
    fn exact_breakpoints_outside_profile_and_zero_crossing_filter() {
        let mut rows = prepare(&[-10, 2, 8], &[3, 4, 20], &[1, 2, 3], 3)
            .unwrap()
            .rows;
        let profile = normalize(&[0, 7], &[5, 12], &[2, 1], 3).unwrap();
        let mut timeline = Timeline::new(&rows, &profile);
        assert_eq!(timeline.endpoints, [-10, 0, 2, 3, 4, 5, 7, 8, 12, 20]);
        assert_eq!(timeline.capacities, [0, 2, 2, 2, 2, 0, 1, 1, 0]);
        filter_impossible(&mut rows, &mut timeline);
        assert_eq!(rows.iter().map(|r| r.index).collect::<Vec<_>>(), [1]);
        assert!(all_feasible(&timeline));
    }

    #[test]
    fn unused_capacity_is_sent_by_the_whole_path_bottleneck() {
        let timeline = Timeline {
            endpoints: vec![0, 1, 2],
            capacities: vec![1_000_000; 2],
            jobs: vec![],
        };
        let mut network = Network::new_transshipment(&timeline, &[]);
        assert_eq!(network.solve().unwrap(), 1);
        assert_eq!(network.edges[1].capacity, 1_000_000);
        assert_eq!(network.edges[3].capacity, 1_000_000);
    }

    #[test]
    fn circulation_keeps_exact_objective_without_summing_saturated_weights() {
        let prepared = prepare(&[0; 5], &[2; 5], &[i128::MAX; 5], 5).unwrap();
        let timeline = Timeline::new(
            &prepared.rows,
            &normalize(&[0, 1], &[1, 2], &[2, 1], 5).unwrap(),
        );
        let mut network = Network::new_circulation(&timeline, &prepared.rows);
        assert_eq!(network.solve().unwrap(), 5);
        let mut mask = prepared.mask;
        network.reconstruct(&prepared.rows, &mut mask);
        assert_eq!(check_objective(&[i128::MAX; 5], &mask).unwrap(), i128::MAX);
    }

    #[test]
    fn adaptive_circulation_handles_frequent_small_capacity_changes() {
        let starts: Vec<_> = (0..512).collect();
        let ends: Vec<_> = starts.iter().map(|&s| s + 32).collect();
        let prepared = prepare(&starts, &ends, &[1; 512], 512).unwrap();
        let profile: Vec<_> = (0..543)
            .map(|i| Segment {
                start: i,
                end: i + 1,
                capacity: 4 + (i as usize % 2),
            })
            .collect();
        let timeline = Timeline::new(&prepared.rows, &profile);
        let mut network = Network::new(&timeline, &prepared.rows);
        assert!(network.circulation);
        assert!(network.solve().unwrap() <= prepared.rows.len());
    }

    #[test]
    fn parallel_components_match_serial_and_restore_shuffled_rows() {
        let mut starts: Vec<_> = (0..16_384).map(|i| i / 32 * 10).collect();
        let mut ends: Vec<_> = starts.iter().map(|s| s + 4).collect();
        let mut weights: Vec<_> = (0..16_384).map(|i| i % 32 + 1).collect();
        starts.reverse();
        ends.reverse();
        weights.reverse();
        let ps: Vec<_> = (0..512).flat_map(|i| [i * 10, i * 10 + 2]).collect();
        let pe: Vec<_> = ps.iter().map(|s| s + 2).collect();
        let capacities: Vec<_> = (0..1024).map(|i| 2 + i % 2).collect();
        let profile = normalize(&ps, &pe, &capacities, starts.len()).unwrap();
        let prepared = prepare(&starts, &ends, &weights, usize::MAX).unwrap();
        let ranges = components(&prepared.rows);
        let expected: Vec<_> = weights.iter().map(|&w| w >= 31).collect();
        for workers in [1, 8] {
            let mut mask = prepared.mask.clone();
            solve_components(&prepared.rows, &profile, &ranges, &mut mask, workers).unwrap();
            assert_eq!(mask, expected);
        }
        for _ in 0..2 {
            assert_eq!(
                max_weight_with_capacity_profile(&starts, &ends, &weights, &ps, &pe, &capacities)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn unreachable_timeline_labels_do_not_overflow_a_representable_optimum() {
        assert_eq!(
            max_weight_with_capacity_profile(
                &[1, 1, 0],
                &[3, 4, 1],
                &[i128::MAX, 1, 1],
                &[0, 1, 2, 3],
                &[1, 2, 3, 4],
                &[0, 2, 1, 2],
            )
            .unwrap(),
            [true, false, false]
        );
        assert_eq!(
            reduced_cost(i128::MAX, -i128::MAX, 0),
            2 * i128::MAX as u128
        );
        assert_eq!(
            reduced_cost(i128::MAX, i128::MAX, i128::MAX),
            i128::MAX as u128
        );
    }

    #[test]
    fn transshipment_cut_conservation_integrality_and_optimality() {
        flow_invariants(
            &[(-6, 4, 15), (-6, -1, 10), (-1, 4, 10)],
            &[1, 1, 2, 2, 1, 1, 3, 3, 2, 2],
        );
        flow_invariants(
            &[(-6, 4, 50), (-6, -3, 10), (0, 4, 10)],
            &[2, 2, 2, 0, 0, 0, 2, 2, 2, 2],
        );
    }

    proptest! {
        #[test]
        fn capacity_tightening_preserves_every_feasible_subset(
            jobs in prop::collection::vec((0usize..=6, 0usize..=6), 0..=8),
            capacities in prop::collection::vec(0usize..=8, 6),
        ) {
            let jobs: Vec<_> = jobs.into_iter().map(|(s, e)| (s.min(e), s.max(e))).filter(|(s, e)| s < e).collect();
            let mut counts = vec![(0usize, 0usize); 7];
            for &(s, e) in &jobs { counts[s].0 += 1; counts[e].1 += 1; }
            let tightened = tighten_capacities(&capacities, &counts);
            prop_assert!(capacity_supply(&tightened) <= jobs.len() as u128);
            for bits in 0usize..1 << jobs.len() {
                let active: Vec<_> = (0..6).map(|t| jobs.iter().enumerate()
                    .filter(|&(i, &(s, e))| bits & (1 << i) != 0 && s <= t && t < e).count()).collect();
                let original_feasible = active.iter().zip(&capacities).all(|(a, c)| a <= c);
                let tightened_feasible = active.iter().zip(&tightened).all(|(a, c)| a <= c);
                prop_assert_eq!(original_feasible, tightened_feasible);
            }
        }

        #[test]
        fn normalization_matches_naive_lookup(
            capacities in prop::collection::vec(0u8..=8, 0..=17),
            omit_zeros in any::<bool>(),
            limit in 0usize..=8,
        ) {
            let mut records: Vec<_> = capacities.iter().enumerate()
                .filter(|(_, c)| !omit_zeros || **c != 0)
                .map(|(i, &c)| (i as i32 - 6, i as i32 - 5, c)).collect();
            records.reverse();
            let starts: Vec<_> = records.iter().map(|r| r.0).collect();
            let ends: Vec<_> = records.iter().map(|r| r.1).collect();
            let values: Vec<_> = records.iter().map(|r| r.2).collect();
            let normalized = normalize(&starts, &ends, &values, limit).unwrap();
            for point in -8..=13 {
                let expected = records.iter().find(|r| r.0 <= point && point < r.1)
                    .map_or(0, |r| usize::from(r.2).min(limit));
                let actual = normalized.iter().find(|r| r.start <= point && point < r.end)
                    .map_or(0, |r| r.capacity);
                prop_assert_eq!(actual, expected);
            }
            for pair in normalized.windows(2) {
                prop_assert_eq!(pair[0].end, pair[1].start);
                prop_assert_ne!(pair[0].capacity, pair[1].capacity);
            }
        }

        #[test]
        fn generated_flow_invariants(
            jobs in prop::collection::vec((-8i32..=10, 0i32..=8, -10i64..=20), 0..=10),
            capacities in prop::collection::vec(0u8..=4, 17),
        ) {
            let rows: Vec<_> = jobs.iter().map(|&(s, len, w)| (s, s + len, w)).collect();
            flow_invariants(&rows, &capacities);
        }

        #[test]
        fn large_exact_weights_match_small_exhaustive_objectives(
            jobs in prop::collection::vec((0i32..=4, 0i32..=4, 0u8..=4), 0..=7),
            capacities in prop::collection::vec(0u8..=3, 4),
        ) {
            let starts: Vec<_> = jobs.iter().map(|r| r.0.min(r.1)).collect();
            let ends: Vec<_> = jobs.iter().map(|r| r.0.max(r.1)).collect();
            let weights: Vec<_> = jobs.iter().map(|r| match r.2 {
                0 => 1, 1 => i128::MAX / 16, 2 => i128::MAX / 4,
                3 => i128::MAX / 2, _ => i128::MAX,
            }).collect();
            let mut optimum = 0u128;
            for bits in 0usize..1 << jobs.len() {
                let feasible = (0..4).all(|t| (0..jobs.len())
                    .filter(|&i| bits & (1 << i) != 0 && starts[i] <= t && t < ends[i])
                    .count() <= usize::from(capacities[t as usize]));
                if feasible {
                    let objective = weights.iter().enumerate()
                        .filter(|(i, _)| bits & (1 << i) != 0)
                        .fold(0u128, |sum, (_, &w)| sum.saturating_add(w as u128));
                    optimum = optimum.max(objective);
                }
            }
            let actual = max_weight_with_capacity_profile(&starts, &ends, &weights,
                &[0, 1, 2, 3], &[1, 2, 3, 4], &capacities);
            if optimum > i128::MAX as u128 {
                prop_assert_eq!(actual, Err(IntervalError::WeightOverflow));
            } else {
                let mask = actual?;
                prop_assert_eq!(check_objective(&weights, &mask).unwrap(), optimum as i128);
            }
            // Exercise both models even when public fast paths or the adaptive
            // selector would avoid them. Drop zero-crossing jobs as production
            // does: rejecting an impossible MAX+MAX chain need not fit i128.
            let mut prepared = prepare(&starts, &ends, &weights, usize::MAX).unwrap();
            let profile = normalize(&[0, 1, 2, 3], &[1, 2, 3, 4], &capacities, jobs.len()).unwrap();
            let mut timeline = Timeline::new(&prepared.rows, &profile);
            filter_impossible(&mut prepared.rows, &mut timeline);
            for circulation in [false, true] {
                let result = (|| {
                    let mut mask = prepared.mask.clone();
                    if !prepared.rows.is_empty() {
                        let mut network = if circulation { Network::new_circulation(&timeline, &prepared.rows) }
                            else { Network::new_transshipment(&timeline, &prepared.rows) };
                        network.solve()?;
                        network.reconstruct(&prepared.rows, &mut mask);
                    }
                    check_objective(&weights, &mask)
                })();
                if optimum > i128::MAX as u128 {
                    prop_assert_eq!(result, Err(IntervalError::WeightOverflow));
                } else {
                    prop_assert_eq!(result?, optimum as i128);
                }
            }
        }
    }
}
