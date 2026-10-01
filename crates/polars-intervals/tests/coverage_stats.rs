use polars::prelude::*;
use polars_intervals::coverage_stats;
use proptest::prelude::*;

mod support;
use support::{endpoint_dtypes, typed_series};

type Stats = (u64, i128, i128, Option<f64>);

fn rows(frame: &DataFrame) -> Vec<Stats> {
    let counts = frame["overlap_count"].u64().unwrap();
    let covered = frame["covered_length"].i128().unwrap();
    let lengths = frame["query_length"].i128().unwrap();
    let fractions = frame["covered_fraction"].f64().unwrap();
    (0..frame.height())
        .map(|i| {
            (
                counts.get(i).unwrap(),
                covered.get(i).unwrap(),
                lengths.get(i).unwrap(),
                fractions.get(i),
            )
        })
        .collect()
}

fn assert_schema(frame: &DataFrame) {
    assert_eq!(
        frame
            .get_column_names()
            .into_iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        [
            "overlap_count",
            "covered_length",
            "query_length",
            "covered_fraction"
        ]
    );
    assert_eq!(
        frame.dtypes(),
        [
            DataType::UInt64,
            DataType::Int128,
            DataType::Int128,
            DataType::Float64
        ]
    );
    for name in ["overlap_count", "covered_length", "query_length"] {
        assert_eq!(frame[name].null_count(), 0);
    }
}

#[test]
fn supported_endpoints_unequal_operands_and_typed_empty_statistics() {
    for dtype in endpoint_dtypes() {
        let qs = typed_series(&[0, 5, 12, 7, 9, 1], &dtype);
        let qe = typed_series(&[10, 10, 15, 7, 12, 7], &dtype);
        let ss = typed_series(&[1, 4], &dtype);
        let se = typed_series(&[7, 9], &dtype);
        let out = coverage_stats(&qs, &qe, &[], &ss, &se, &[]).unwrap();
        assert_schema(&out);
        assert_eq!(
            rows(&out),
            [
                (2, 8, 10, Some(0.8)),
                (2, 4, 5, Some(0.8)),
                (0, 0, 3, Some(0.0)),
                (0, 0, 0, None),
                (0, 0, 3, Some(0.0)),
                (2, 6, 6, Some(1.0))
            ]
        );
        let empty = coverage_stats(&qs.clear(), &qe.clear(), &[], &ss, &se, &[]).unwrap();
        assert_schema(&empty);
        assert_eq!(empty.height(), 0);
        let empty_sources = coverage_stats(&qs, &qe, &[], &ss.clear(), &se.clear(), &[]).unwrap();
        assert_eq!(
            rows(&empty_sources),
            [
                (0, 0, 10, Some(0.0)),
                (0, 0, 5, Some(0.0)),
                (0, 0, 3, Some(0.0)),
                (0, 0, 0, None),
                (0, 0, 3, Some(0.0)),
                (0, 0, 6, Some(0.0))
            ]
        );
    }
}

#[test]
fn composite_null_keys_interleaved_queries_preserve_global_order() {
    let qs = Series::new("s".into(), [0i64; 7]);
    let qe = Series::new("e".into(), [10i64; 7]);
    let qk = [
        Series::new(
            "start".into(),
            [
                Some("b"),
                None,
                Some("a"),
                Some("b"),
                Some("missing"),
                None,
                Some("a"),
            ],
        ),
        Series::new(
            "end".into(),
            [Some(1i32), None, Some(2), Some(1), None, Some(3), Some(2)],
        ),
    ];
    let ss = Series::new("s".into(), [4i64, 1, 7, 0, 8, 5]);
    let se = Series::new("e".into(), [6i64, 4, 9, 10, 10, 5]);
    let sk = [
        Series::new(
            "start".into(),
            [
                Some("a"),
                Some("b"),
                None,
                Some("source-only"),
                Some("b"),
                None,
            ],
        ),
        Series::new(
            "end".into(),
            [Some(2i32), Some(1), None, Some(1), Some(1), None],
        ),
    ];
    let out = coverage_stats(&qs, &qe, &qk, &ss, &se, &sk).unwrap();
    assert_eq!(
        rows(&out),
        [
            (2, 5, 10, Some(0.5)),
            (1, 2, 10, Some(0.2)),
            (1, 2, 10, Some(0.2)),
            (2, 5, 10, Some(0.5)),
            (0, 0, 10, Some(0.0)),
            (0, 0, 10, Some(0.0)),
            (1, 2, 10, Some(0.2))
        ]
    );
}

#[test]
fn every_supported_group_key_dtype_matches_nulls() {
    let qs = Series::new("s".into(), [0i64; 3]);
    let qe = Series::new("e".into(), [10i64; 3]);
    let ss = Series::new("s".into(), [2i64, 6]);
    let se = Series::new("e".into(), [5i64, 8]);
    let mut keys = endpoint_dtypes()
        .into_iter()
        .map(|dtype| {
            let mut qk = typed_series(&[0, 1], &dtype);
            qk.append(&Series::full_null("endpoint".into(), 1, &dtype))
                .unwrap();
            let mut sk = typed_series(&[1], &dtype);
            sk.append(&Series::full_null("endpoint".into(), 1, &dtype))
                .unwrap();
            (qk, sk)
        })
        .collect::<Vec<_>>();
    keys.extend([
        (
            Series::new("key".into(), [Some("a"), Some("b"), None]),
            Series::new("key".into(), [Some("b"), None]),
        ),
        (
            Series::new("key".into(), [Some(false), Some(true), None]),
            Series::new("key".into(), [Some(true), None]),
        ),
    ]);
    for (qk, sk) in keys {
        let out = coverage_stats(&qs, &qe, &[qk], &ss, &se, &[sk]).unwrap();
        assert_eq!(
            rows(&out),
            [
                (0, 0, 10, Some(0.0)),
                (1, 3, 10, Some(0.3)),
                (1, 2, 10, Some(0.2))
            ]
        );
    }
}

#[test]
fn full_width_lengths_and_small_suffixes_remain_exact() {
    for (qs, qe, ss, se, expected) in [
        (
            Series::new("s".into(), [i64::MIN, i64::MAX - 1, i64::MAX - 2]),
            Series::new("e".into(), [i64::MAX, i64::MAX, i64::MAX - 1]),
            Series::new("s".into(), [i64::MIN, i64::MAX - 1]),
            Series::new("e".into(), [i64::MAX - 2, i64::MAX]),
            vec![
                (2, u64::MAX as i128 - 1, u64::MAX as i128, Some(1.0)),
                (1, 1, 1, Some(1.0)),
                (0, 0, 1, Some(0.0)),
            ],
        ),
        (
            Series::new("s".into(), [0u64, u64::MAX - 1, u64::MAX - 2]),
            Series::new("e".into(), [u64::MAX, u64::MAX, u64::MAX - 1]),
            Series::new("s".into(), [0u64, u64::MAX - 1]),
            Series::new("e".into(), [u64::MAX - 2, u64::MAX]),
            vec![
                (2, u64::MAX as i128 - 1, u64::MAX as i128, Some(1.0)),
                (1, 1, 1, Some(1.0)),
                (0, 0, 1, Some(0.0)),
            ],
        ),
        (
            Series::new("s".into(), [i8::MIN]),
            Series::new("e".into(), [i8::MAX]),
            Series::new("s".into(), [i8::MIN]),
            Series::new("e".into(), [i8::MAX]),
            vec![(1, 255, 255, Some(1.0))],
        ),
    ] {
        assert_eq!(
            rows(&coverage_stats(&qs, &qe, &[], &ss, &se, &[]).unwrap()),
            expected
        );
    }
}

#[test]
fn validates_original_rows_before_empty_and_unmatched_shortcuts() {
    let starts = Series::new("s".into(), [0i64, 1, 20, 30]);
    let ends = Series::new("e".into(), [0i64, 1, 20, 29]);
    let query_keys = Series::new("key".into(), ["query-only"; 4]);
    let interval_keys = Series::new("key".into(), ["source-only"; 4]);
    for (qs, qe, keys) in [
        (starts.clone(), starts.clone(), query_keys.clone()),
        (starts.clear(), starts.clear(), query_keys.clear()),
    ] {
        let error = coverage_stats(
            &qs,
            &qe,
            &[keys],
            &starts,
            &ends,
            std::slice::from_ref(&interval_keys),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("intervals: interval at index 3"),
            "{error}"
        );
    }
    let error = coverage_stats(
        &starts,
        &ends,
        &[query_keys],
        &starts,
        &starts,
        &[interval_keys],
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("queries: interval at index 3"),
        "{error}"
    );
    let null_ends = Series::new("e".into(), [Some(0i64), Some(1), None, Some(30)]);
    for (qs, qe, ss, se, side) in [
        (&starts, &null_ends, &starts, &ends, "queries"),
        (&starts, &starts, &starts, &null_ends, "intervals"),
    ] {
        let error = coverage_stats(qs, qe, &[], ss, se, &[]).unwrap_err();
        assert!(
            error.to_string().contains(&format!(
                "{side} does not support null endpoints at index 2"
            )),
            "{error}"
        );
    }
}

#[test]
fn schemas_match_across_typed_empty_operands_and_keys_never_cast() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let ends = Series::new("e".into(), [2i64, 3]);
    let key = Series::new("key".into(), [0i64, 1]);
    for (left, right) in [
        (vec![key.clone()], vec![]),
        (
            vec![key.clone()],
            vec![Series::new("other".into(), [0i64, 1])],
        ),
        (
            vec![key.clone()],
            vec![Series::new("key".into(), [0i32, 1])],
        ),
        (vec![key.slice(0, 1)], vec![key.clone()]),
        (
            vec![Series::new("key".into(), [0.0f64, 1.0])],
            vec![Series::new("key".into(), [0.0f64, 1.0])],
        ),
        (vec![key.clone(); 2], vec![key.clone(); 2]),
        (
            vec![Series::new("covered_length".into(), [0i64, 1])],
            vec![Series::new("covered_length".into(), [0i64, 1])],
        ),
    ] {
        assert!(coverage_stats(&starts, &ends, &left, &starts, &ends, &right).is_err());
    }
    for dtype in endpoint_dtypes()
        .into_iter()
        .filter(|dtype| *dtype != DataType::Int64)
    {
        let other = typed_series(&[], &dtype);
        let error =
            coverage_stats(&starts.clear(), &ends.clear(), &[], &other, &other, &[]).unwrap_err();
        assert!(
            error.to_string().contains("across queries and intervals"),
            "{error}"
        );
    }
}

// Deliberately inspect original rows and elementary cells. No production union,
// endpoint ranks, or prefix preparation participates in this oracle.
fn cell_oracle(query: (i64, i64), sources: &[(i64, i64)]) -> Stats {
    let (a, b) = query;
    if a == b {
        return (0, 0, 0, None);
    }
    let count = sources
        .iter()
        .filter(|&&(s, e)| s < e && s < b && a < e)
        .count() as u64;
    let mut boundaries = vec![a, b];
    for &(s, e) in sources {
        if s < e && s < b && a < e {
            boundaries.extend([s.max(a), e.min(b)]);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let covered = boundaries
        .windows(2)
        .filter(|cell| sources.iter().any(|&(s, e)| s <= cell[0] && cell[1] <= e))
        .map(|cell| i128::from(cell[1]) - i128::from(cell[0]))
        .sum::<i128>();
    let length = i128::from(b) - i128::from(a);
    (count, covered, length, Some(covered as f64 / length as f64))
}

proptest! {
    #[test]
    fn grouped_chunks_match_independent_original_row_oracle(
        queries in prop::collection::vec((-12i64..=12, -12i64..=12, prop::option::of(0i64..4)), 0..35),
        intervals in prop::collection::vec((-12i64..=12, -12i64..=12, prop::option::of(0i64..5)), 0..35),
        cuts in prop::array::uniform6(0usize..36),
    ) {
        let make = |raw: &[(i64,i64,Option<i64>)]| (
            Series::new("s".into(), raw.iter().map(|r| r.0.min(r.1)).collect::<Vec<_>>()),
            Series::new("e".into(), raw.iter().map(|r| r.0.max(r.1)).collect::<Vec<_>>()),
            Series::new("key".into(), raw.iter().map(|r| r.2).collect::<Vec<_>>()),
        );
        let (qs,qe,qk) = make(&queries);
        let (ss,se,sk) = make(&intervals);
        let expected = queries.iter().map(|q| {
            let sources = intervals.iter().filter(|s| s.2==q.2).map(|s| (s.0.min(s.1),s.0.max(s.1))).collect::<Vec<_>>();
            cell_oracle((q.0.min(q.1),q.0.max(q.1)),&sources)
        }).collect::<Vec<_>>();
        let result = coverage_stats(&qs,&qe,std::slice::from_ref(&qk),&ss,&se,std::slice::from_ref(&sk)).unwrap();
        prop_assert_eq!(rows(&result),expected);
        let chunk = |series: &Series, cut: usize| {
            let cut = cut % (series.len()+1);
            let mut result = series.slice(0,cut);
            result.append(&series.slice(cut as i64, series.len()-cut)).unwrap();
            result
        };
        let chunked = coverage_stats(&chunk(&qs,cuts[0]),&chunk(&qe,cuts[1]),&[chunk(&qk,cuts[2])],&chunk(&ss,cuts[3]),&chunk(&se,cuts[4]),&[chunk(&sk,cuts[5])]).unwrap();
        prop_assert!(result.equals_missing(&chunked));
    }
}
