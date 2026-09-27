use crate::IntervalError;

/// Counts how many other intervals each row contains, in original row order.
///
/// For half-open intervals, containment is defined by the endpoint predicate
/// `a.start <= b.start && b.end <= a.end`, excluding only the same row.
/// Duplicates count one another. Empty intervals follow the same inequalities:
/// `[0, 5)` contains `[5, 5)`, and two `[5, 5)` rows contain each other.
/// An empty outer interval contains only empties at its own coordinate.
/// This differs from overlap counting, where empty intervals overlap nothing.
///
/// A descending-start sweep over packed records uses compressed ends and a
/// private Fenwick tree. The complete equal-start group is inserted before any
/// of its queries. Time is `O(n log n)` and additional space is `O(n)`; no pairs
/// are materialized and no arithmetic is performed on endpoint values.
///
/// # Errors
///
/// Returns [`IntervalError::LengthMismatch`] for unequal slice lengths, or
/// [`IntervalError::InvalidInterval`] for the first original row with `start > end`.
///
/// # Examples
///
/// ```
/// use intervals_core::containment_counts;
/// assert_eq!(containment_counts(&[0, 2, 4], &[10, 5, 12])?, [1, 0, 0]);
/// assert_eq!(containment_counts(&[0, 5, 5], &[5, 5, 5])?, [2, 1, 1]);
/// # Ok::<(), intervals_core::IntervalError>(())
/// ```
pub fn containment_counts<T: Ord + Copy>(
    starts: &[T],
    ends: &[T],
) -> Result<Vec<usize>, IntervalError> {
    if starts.len() != ends.len() {
        return Err(IntervalError::LengthMismatch {
            starts_len: starts.len(),
            ends_len: ends.len(),
        });
    }
    let mut records = Vec::with_capacity(starts.len());
    for (index, (&start, &end)) in starts.iter().zip(ends).enumerate() {
        if start > end {
            return Err(IntervalError::InvalidInterval { index });
        }
        records.push((start, end, index));
    }
    let mut coords = ends.to_vec();
    coords.sort_unstable();
    coords.dedup();
    // Reuse the output buffer for ranks until each row's query is complete.
    let mut counts: Vec<_> = ends
        .iter()
        .map(|end| coords.binary_search(end).unwrap())
        .collect();
    records.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut tree = Fenwick::new(coords.len());
    let mut first = 0;
    while first < records.len() {
        let mut last = first + 1;
        while last < records.len() && records[last].0 == records[first].0 {
            last += 1;
        }
        // At query time the tree contains exactly all rows with start >= this
        // start, including the entire tie group, irrespective of end ordering.
        for &(_, _, index) in &records[first..last] {
            tree.add(counts[index]);
        }
        for &(_, _, index) in &records[first..last] {
            // Inclusive end prefix includes this row exactly once. All counters
            // are bounded by starts.len(), so usize additions cannot overflow.
            counts[index] = tree.prefix(counts[index] + 1) - 1;
        }
        first = last;
    }
    Ok(counts)
}

struct Fenwick(Vec<usize>);

impl Fenwick {
    fn new(size: usize) -> Self {
        Self(vec![0; size + 1])
    }

    fn add(&mut self, index: usize) {
        let mut i = index + 1;
        while i < self.0.len() {
            self.0[i] += 1;
            i += i.isolate_lowest_one();
        }
    }

    // Sum of zero-based coordinates [0, end).
    fn prefix(&self, mut end: usize) -> usize {
        let mut total = 0;
        while end > 0 {
            total += self.0[end];
            end &= end - 1;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::Fenwick;
    use proptest::prelude::*;

    #[test]
    fn empty_tree() {
        assert_eq!(Fenwick::new(0).prefix(0), 0);
    }
    #[test]
    fn first_last_repeated_updates_and_prefixes() {
        let mut tree = Fenwick::new(5);
        tree.add(0);
        tree.add(4);
        tree.add(4);
        tree.add(2);
        assert_eq!(
            (0..=5).map(|i| tree.prefix(i)).collect::<Vec<_>>(),
            [0, 1, 1, 2, 2, 4]
        );
    }
    #[test]
    fn duplicate_ends_all_coordinates_equal() {
        let mut tree = Fenwick::new(1);
        for _ in 0..100 {
            tree.add(0);
        }
        assert_eq!(tree.prefix(1), 100);
    }
    proptest! {
        #[test]
        fn random_operations_match_array(
            n in 1usize..=32,
            ops in prop::collection::vec((any::<usize>(), any::<bool>()), 0..200),
        ) {
            let mut tree = Fenwick::new(n);
            let mut array = vec![0usize; n];
            for (i, update) in ops {
                if update {
                    tree.add(i % n);
                    array[i % n] += 1;
                } else {
                    let end = i % (n + 1);
                    prop_assert_eq!(tree.prefix(end), array[..end].iter().sum::<usize>());
                }
            }
        }
    }
}
