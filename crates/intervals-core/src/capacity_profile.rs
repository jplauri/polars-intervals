use crate::capacity::{Prepared, Row, check_objective, components, prepare};
use crate::capacity_flow::{Network, Segment, Timeline, solve_components};
use crate::{IntervalError, check_len, max_weight_with_capacity};

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
                solve_components(&rows, &ranges, &mut mask, workers, |component, emit| {
                    solve_component(component, &profile, emit)
                })?;
            }
        }
    }
    check_objective(weights, &mask)?;
    Ok(mask)
}

fn solve_component<T: Ord + Copy>(
    rows: &[Row<T>],
    profile: &[Segment<T>],
    emit: &mut dyn FnMut(usize),
) -> Result<(), IntervalError> {
    let timeline = Timeline::new(rows, profile);
    if all_feasible(&timeline) {
        rows.iter().for_each(|row| emit(row.index));
        return Ok(());
    }
    Network::new(&timeline, rows).select(rows, emit)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity_flow::reduced_cost;
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
    fn sink_distances_past_the_u128_range_report_overflow_in_both_models() {
        let solve = |jobs: &[(i32, i32, i128)], capacities: &[u8], circulation| {
            let starts: Vec<_> = jobs.iter().map(|j| j.0).collect();
            let ends: Vec<_> = jobs.iter().map(|j| j.1).collect();
            let weights: Vec<_> = jobs.iter().map(|j| j.2).collect();
            let prepared = prepare(&starts, &ends, &weights, usize::MAX).unwrap();
            let points: Vec<_> = (0..capacities.len() as i32).collect();
            let after: Vec<_> = points.iter().map(|p| p + 1).collect();
            let profile = normalize(&points, &after, capacities, jobs.len()).unwrap();
            let timeline = Timeline::new(&prepared.rows, &profile);
            if circulation {
                Network::new_circulation(&timeline, &prepared.rows).solve()
            } else {
                Network::new_transshipment(&timeline, &prepared.rows).solve()
            }
        };
        let max = i128::MAX;
        // The last transshipment path needs more than u128::MAX of reduced cost.
        assert_eq!(
            solve(
                &[(2, 3, max), (3, 6, 1), (3, 4, 1), (0, 3, max)],
                &[2, 2, 1, 1, 2, 2],
                false
            ),
            Err(IntervalError::WeightOverflow)
        );
        // Circulation saturates on its rejection chain before reaching the sink.
        assert_eq!(
            solve(
                &[
                    (0, 1, max),
                    (0, 1, max - 1),
                    (1, 2, max),
                    (2, 3, max),
                    (1, 2, max),
                    (2, 3, max - 1)
                ],
                &[1, 1, 1],
                true
            ),
            Err(IntervalError::WeightOverflow)
        );
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
            solve_components(
                &prepared.rows,
                &ranges,
                &mut mask,
                workers,
                |component, emit| solve_component(component, &profile, emit),
            )
            .unwrap();
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
        fn one_segment_network_is_the_adaptive_choice(
            rows in prop::collection::vec((-4i32..=4, 1i32..=6), 1..=20),
            capacity in 1usize..=8,
        ) {
            let starts: Vec<_> = rows.iter().map(|r| r.0).collect();
            let ends: Vec<_> = rows.iter().map(|r| r.0 + r.1).collect();
            let prepared = prepare(&starts, &ends, &vec![1i64; rows.len()], capacity).unwrap();
            let rows = &prepared.rows;
            let segment = Segment {
                start: rows[0].start,
                end: rows.iter().map(|r| r.end).max().unwrap(),
                capacity,
            };
            let timeline = Timeline::new(rows, &[segment]);
            if !all_feasible(&timeline) {
                let arcs = |network: &Network| {
                    network.edges.iter().map(|e| (e.to, e.capacity, e.cost)).collect::<Vec<_>>()
                };
                let adaptive = Network::new(&timeline, rows);
                let direct = Network::one_segment(rows, capacity);
                prop_assert!(!adaptive.circulation);
                prop_assert_eq!(adaptive.required, direct.required);
                prop_assert_eq!(arcs(&adaptive), arcs(&direct));
            }
        }

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
