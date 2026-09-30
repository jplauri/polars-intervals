use polars::prelude::*;
use polars_intervals::{cluster_intervals, interval_gaps, merge_intervals};
use proptest::prelude::*;

mod support;
use support::{endpoint_dtypes, typed_series};

fn bounds(dtype: &DataType, left: i64, right: i64) -> (Scalar, Scalar) {
    let values = typed_series(&[left, right], dtype);
    (
        Scalar::new(dtype.clone(), values.get(0).unwrap().into_static()),
        Scalar::new(dtype.clone(), values.get(1).unwrap().into_static()),
    )
}

fn ranges(frame: &DataFrame) -> Vec<(i64, i64)> {
    let starts = frame["start"]
        .to_physical_repr()
        .cast(&DataType::Int64)
        .unwrap();
    let ends = frame["end"]
        .to_physical_repr()
        .cast(&DataType::Int64)
        .unwrap();
    starts
        .i64()
        .unwrap()
        .into_no_null_iter()
        .zip(ends.i64().unwrap().into_no_null_iter())
        .collect()
}

#[test]
fn every_endpoint_dtype_preserves_exact_schema_and_empty_results() {
    for dtype in endpoint_dtypes() {
        let starts = typed_series(&[4, 0, 2, 2, 8, 9], &dtype);
        let ends = typed_series(&[6, 2, 4, 2, 10, 9], &dtype);
        for (touching, expected) in [(false, [0, 1, 2, 3, 4, 5]), (true, [0, 0, 0, 1, 2, 3])] {
            let labels = cluster_intervals(&starts, &ends, touching).unwrap();
            assert_eq!(labels.dtype(), &DataType::UInt32);
            assert_eq!(labels.null_count(), 0);
            assert_eq!(
                labels
                    .u32()
                    .unwrap()
                    .into_no_null_iter()
                    .collect::<Vec<_>>(),
                expected
            );
            let empty = cluster_intervals(&starts.clear(), &ends.clear(), touching).unwrap();
            assert_eq!(empty.dtype(), &DataType::UInt32);
            assert!(empty.is_empty());
        }
        let (left, right) = bounds(&dtype, 0, 12);
        for (result, expected) in [
            (
                merge_intervals(&starts, &ends, &[]).unwrap(),
                vec![(0, 6), (8, 10)],
            ),
            (
                interval_gaps(&starts, &ends, &[], &left, &right).unwrap(),
                vec![(6, 8), (10, 12)],
            ),
            (merge_intervals(&starts, &starts, &[]).unwrap(), vec![]),
            (
                interval_gaps(&starts, &ends, &[], &left, &left).unwrap(),
                vec![],
            ),
            (
                interval_gaps(&starts.clear(), &ends.clear(), &[], &left, &right).unwrap(),
                vec![(0, 12)],
            ),
        ] {
            assert_eq!(result.get_column_names(), ["start", "end"]);
            assert_eq!(result["start"].dtype(), &dtype);
            assert_eq!(result["end"].dtype(), &dtype);
            assert_eq!(result["start"].null_count(), 0);
            assert_eq!(result["end"].null_count(), 0);
            assert_eq!(ranges(&result), expected);
        }
    }
}

#[test]
fn stable_nullable_groups_include_empty_and_clipped_groups_across_chunks() {
    let mut starts = Series::new("s".into(), [20i64, 0, 1]);
    starts
        .append(&Series::new("s".into(), [4i64, 8, 21]))
        .unwrap();
    let mut ends = Series::new("e".into(), [20i64]);
    ends.append(&Series::new("e".into(), [3i64, 2, 5, 8, 22]))
        .unwrap();
    let key = Series::new(
        "key".into(),
        [Some("b"), None, Some("a"), None, Some("empty"), Some("b")],
    );
    let flag = Series::new("flag".into(), [true, false, true, false, true, true]);
    let keys = [key, flag];
    let merged = merge_intervals(&starts, &ends, &keys).unwrap();
    assert_eq!(merged.get_column_names(), ["key", "flag", "start", "end"]);
    assert_eq!(ranges(&merged), [(21, 22), (0, 3), (4, 5), (1, 2)]);
    assert_eq!(
        merged["key"].str().unwrap().iter().collect::<Vec<_>>(),
        [Some("b"), None, None, Some("a")]
    );
    let left = Scalar::from(0i64);
    let right = Scalar::from(6i64);
    let gaps = interval_gaps(&starts, &ends, &keys, &left, &right).unwrap();
    assert_eq!(
        ranges(&gaps),
        [(0, 6), (3, 4), (5, 6), (0, 1), (2, 6), (0, 6)]
    );
    assert_eq!(
        gaps["key"].str().unwrap().iter().collect::<Vec<_>>(),
        [Some("b"), None, None, Some("a"), Some("a"), Some("empty")]
    );
    let rechunked = keys.iter().map(|key| key.rechunk()).collect::<Vec<_>>();
    assert!(
        merged.equals_missing(
            &merge_intervals(&starts.rechunk(), &ends.rechunk(), &rechunked).unwrap()
        )
    );
    assert!(
        gaps.equals_missing(
            &interval_gaps(
                &starts.rechunk(),
                &ends.rechunk(),
                &rechunked,
                &left,
                &right
            )
            .unwrap()
        )
    );
}

#[test]
fn every_supported_key_dtype_survives_empty_output_and_absent_groups() {
    let starts = Series::new("s".into(), [1i64, 2]);
    let (left, right) = (Scalar::from(0i64), Scalar::from(5i64));
    for dtype in endpoint_dtypes() {
        let mut key = typed_series(&[1, 2], &dtype);
        key.rename("key".into());
        let keys = [key];
        let empty = merge_intervals(&starts, &starts, &keys).unwrap();
        assert_eq!(empty.height(), 0);
        assert_eq!(empty["key"].dtype(), &dtype);
        let gaps = interval_gaps(&starts, &starts, &keys, &left, &right).unwrap();
        assert_eq!(ranges(&gaps), [(0, 5), (0, 5)]);
        assert_eq!(gaps["key"].dtype(), &dtype);
        let absent = interval_gaps(
            &starts.clear(),
            &starts.clear(),
            &[keys[0].clear()],
            &left,
            &right,
        )
        .unwrap();
        assert_eq!(absent.height(), 0);
        assert_eq!(absent["key"].dtype(), &dtype);
    }
}

#[test]
fn uint64_boundaries_are_exact_and_need_no_arithmetic() {
    let high = u64::MAX;
    let starts = Series::new("s".into(), [high - 4, high - 2, 0]);
    let ends = Series::new("e".into(), [high - 2, high, 1]);
    let merged = merge_intervals(&starts, &ends, &[]).unwrap();
    assert_eq!(
        merged["start"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [0, high - 4]
    );
    assert_eq!(
        merged["end"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [1, high]
    );
    let gaps = interval_gaps(
        &starts,
        &ends,
        &[],
        &Scalar::from(0u64),
        &Scalar::from(high),
    )
    .unwrap();
    assert_eq!(gaps["start"].u64().unwrap().get(0), Some(1));
    assert_eq!(gaps["end"].u64().unwrap().get(0), Some(high - 4));
}

#[test]
fn invalid_rows_keep_original_indices_even_for_empty_domain_and_groups() {
    let starts = Series::new("s".into(), [0i64, 1, 20, 30]);
    let ends = Series::new("e".into(), [0i64, 1, 20, 29]);
    let key = Series::new("key".into(), ["a", "b", "b", "a"]);
    let zero = Scalar::from(0i64);
    for keys in [&[][..], &[key][..]] {
        for result in [
            merge_intervals(&starts, &ends, keys),
            interval_gaps(&starts, &ends, keys, &zero, &zero),
        ] {
            assert!(result.unwrap_err().to_string().contains("index 3"));
        }
    }
}

#[test]
fn keys_and_scalar_bounds_are_strict() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let ends = Series::new("e".into(), [2i64, 3]);
    for keys in [
        vec![Series::new("start".into(), [0i64, 1])],
        vec![Series::new("end".into(), [0i64, 1])],
        vec![
            Series::new("k".into(), [0i64, 1]),
            Series::new("k".into(), [0i64, 1]),
        ],
        vec![Series::new("k".into(), [0.0f64, 1.0])],
        vec![Series::new("k".into(), [0i64])],
    ] {
        assert!(merge_intervals(&starts, &ends, &keys).is_err());
    }
    let right = Scalar::from(5i64);
    for left in [
        Scalar::from(0i32),
        Scalar::from(6i64),
        Scalar::new(DataType::Int64, AnyValue::Null),
    ] {
        assert!(interval_gaps(&starts, &ends, &[], &left, &right).is_err());
    }
    // Only endpoint output names are reserved for geometry.
    assert!(merge_intervals(&starts, &ends, &[Series::new("load".into(), [1i64, 1])]).is_ok());
}

fn cell_oracle(rows: &[(i64, i64)], domain: Option<(i64, i64)>) -> Vec<(i64, i64)> {
    let mut boundaries = rows.iter().flat_map(|&(s, e)| [s, e]).collect::<Vec<_>>();
    if let Some((left, right)) = domain {
        boundaries.extend([left, right]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut output: Vec<(i64, i64)> = Vec::new();
    for cell in boundaries.windows(2) {
        let (left, right) = (cell[0], cell[1]);
        let covered = rows.iter().any(|&(s, e)| s <= left && left < e);
        let keep = domain.map_or(covered, |(a, b)| a <= left && right <= b && !covered);
        if keep {
            if let Some(last) = output.last_mut().filter(|last| last.1 == left) {
                last.1 = right;
            } else {
                output.push((left, right));
            }
        }
    }
    output
}

proptest! {
    #[test]
    fn native_grouping_matches_independent_cells(
        raw in prop::collection::vec((-12i64..=12, -12i64..=12, prop::option::of(0i64..3)), 0..30),
        a in -15i64..=15,
        b in -15i64..=15,
        cuts in prop::array::uniform6(0usize..31),
    ) {
        let rows = raw.iter().map(|&(s,e,_)| (s.min(e), s.max(e))).collect::<Vec<_>>();
        let starts = Series::new("s".into(), rows.iter().map(|r| r.0).collect::<Vec<_>>());
        let ends = Series::new("e".into(), rows.iter().map(|r| r.1).collect::<Vec<_>>());
        let key = Series::new("key".into(), raw.iter().map(|r| r.2).collect::<Vec<_>>());
        let keys = [key];
        let repartition = |series: &Series, first: usize, second: usize| {
            let (first, second) = (first % (series.len() + 1), second % (series.len() + 1));
            let (first, second) = (first.min(second), first.max(second));
            let mut result = series.slice(0, first);
            result.append(&series.slice(first as i64, second - first)).unwrap();
            result.append(&series.slice(second as i64, series.len() - second)).unwrap();
            result
        };
        let chunked_starts = repartition(&starts, cuts[0], cuts[1]);
        let chunked_ends = repartition(&ends, cuts[2], cuts[3]);
        let chunked_keys = [repartition(&keys[0], cuts[4], cuts[5])];
        let domain = (a.min(b), a.max(b));
        let mut observed = Vec::new();
        for row in &raw { if !observed.contains(&row.2) { observed.push(row.2); } }
        for bounded in [false, true] {
            let result = if bounded {
                interval_gaps(&starts, &ends, &keys, &Scalar::from(domain.0), &Scalar::from(domain.1)).unwrap()
            } else { merge_intervals(&starts, &ends, &keys).unwrap() };
            let expected = observed.iter().flat_map(|key| {
                let group = rows.iter().zip(&raw).filter(|(_,r)| r.2 == *key).map(|(r,_)| *r).collect::<Vec<_>>();
                cell_oracle(&group, bounded.then_some(domain)).into_iter().map(|r| (*key, r))
            }).collect::<Vec<_>>();
            let actual = result["key"].i64().unwrap().iter().zip(ranges(&result)).collect::<Vec<_>>();
            prop_assert_eq!(&actual, &expected);
            let chunked = if bounded {
                interval_gaps(&chunked_starts, &chunked_ends, &chunked_keys, &Scalar::from(domain.0), &Scalar::from(domain.1)).unwrap()
            } else { merge_intervals(&chunked_starts, &chunked_ends, &chunked_keys).unwrap() };
            let actual = chunked["key"].i64().unwrap().iter().zip(ranges(&chunked)).collect::<Vec<_>>();
            prop_assert_eq!(actual, expected);
            prop_assert!(result.equals_missing(&chunked));
        }
    }
}
