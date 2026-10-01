use polars::prelude::*;
use polars_intervals::{intersect_intervals, subtract_intervals};
use proptest::prelude::*;

mod support;
use support::{endpoint_dtypes, typed_series};

type Operation =
    fn(&Series, &Series, &[Series], &Series, &Series, &[Series]) -> PolarsResult<DataFrame>;
const OPERATIONS: [Operation; 2] = [subtract_intervals, intersect_intervals];

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
fn supported_endpoints_preserve_exact_schema_with_unequal_and_empty_inputs() {
    for dtype in endpoint_dtypes() {
        let ls = typed_series(&[0, 4, 12], &dtype);
        let le = typed_series(&[5, 10, 15], &dtype);
        let rs = typed_series(&[2, 6, 10, 30], &dtype);
        let re = typed_series(&[3, 8, 13, 30], &dtype);
        for (operation, expected) in [
            (OPERATIONS[0], vec![(0, 2), (3, 6), (8, 10), (13, 15)]),
            (OPERATIONS[1], vec![(2, 3), (6, 8), (12, 13)]),
        ] {
            let result = operation(&ls, &le, &[], &rs, &re, &[]).unwrap();
            assert_eq!(ranges(&result), expected);
            assert_eq!(result.get_column_names(), ["start", "end"]);
            assert_eq!(result["start"].dtype(), &dtype);
            assert_eq!(result["end"].dtype(), &dtype);
            for empty in [
                operation(&ls.clear(), &le.clear(), &[], &rs, &re, &[]).unwrap(),
                operation(&ls.clear(), &le.clear(), &[], &rs.clear(), &re.clear(), &[]).unwrap(),
                operation(&ls, &ls, &[], &rs, &rs, &[]).unwrap(),
            ] {
                assert_eq!(empty.height(), 0);
                assert_eq!(empty["start"].dtype(), &dtype);
                assert_eq!(empty["end"].dtype(), &dtype);
            }
        }
        let difference = subtract_intervals(&ls, &le, &[], &rs.clear(), &re.clear(), &[]).unwrap();
        assert_eq!(ranges(&difference), [(0, 10), (12, 15)]);
        assert_eq!(
            intersect_intervals(&ls, &le, &[], &rs.clear(), &re.clear(), &[])
                .unwrap()
                .height(),
            0
        );
    }
}

#[test]
fn nullable_keys_and_empty_first_rows_preserve_left_group_order() {
    let ls = Series::new("ls".into(), [20i64, 0, 1, 4, 8, 21]);
    let le = Series::new("le".into(), [20i64, 3, 2, 7, 8, 24]);
    let lk = [
        Series::new(
            "key".into(),
            [Some("b"), None, Some("a"), None, Some("empty"), Some("b")],
        ),
        Series::new("flag".into(), [true, false, true, false, true, true]),
    ];
    let rs = Series::new("rs".into(), [1i64, 2, 0, 22, 9]);
    let re = Series::new("re".into(), [2i64, 5, 100, 23, 9]);
    let rk = [
        Series::new(
            "key".into(),
            [
                Some("a"),
                None,
                Some("right-only"),
                Some("b"),
                Some("empty"),
            ],
        ),
        Series::new("flag".into(), [true, false, true, true, true]),
    ];
    let difference = subtract_intervals(&ls, &le, &lk, &rs, &re, &rk).unwrap();
    assert_eq!(
        difference.get_column_names(),
        ["key", "flag", "start", "end"]
    );
    assert_eq!(ranges(&difference), [(21, 22), (23, 24), (0, 2), (5, 7)]);
    assert_eq!(
        difference["key"].str().unwrap().iter().collect::<Vec<_>>(),
        [Some("b"), Some("b"), None, None]
    );
    let intersection = intersect_intervals(&ls, &le, &lk, &rs, &re, &rk).unwrap();
    assert_eq!(ranges(&intersection), [(22, 23), (2, 3), (4, 5), (1, 2)]);
    assert_eq!(
        intersection["key"]
            .str()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [Some("b"), None, None, Some("a")]
    );
}

#[test]
fn all_supported_key_dtypes_preserve_left_schema_even_for_empty_outputs() {
    let starts = Series::new("s".into(), [0i64, 10]);
    let ends = Series::new("e".into(), [3i64, 13]);
    for dtype in endpoint_dtypes()
        .into_iter()
        .chain([DataType::Boolean, DataType::String])
    {
        let key = match dtype {
            DataType::Boolean => Series::new("key".into(), [false, true]),
            DataType::String => Series::new("key".into(), ["b", "a"]),
            _ => {
                let mut key = typed_series(&[0, 1], &dtype);
                key.rename("key".into());
                key
            }
        };
        for operation in OPERATIONS {
            let result = operation(
                &starts,
                &ends,
                std::slice::from_ref(&key),
                &starts,
                &ends,
                std::slice::from_ref(&key),
            )
            .unwrap();
            assert_eq!(result["key"].dtype(), &dtype);
            let empty = operation(
                &starts.clear(),
                &ends.clear(),
                &[key.clear()],
                &starts,
                &ends,
                std::slice::from_ref(&key),
            )
            .unwrap();
            assert_eq!(empty.height(), 0);
            assert_eq!(empty["key"].dtype(), &dtype);
        }
    }
}

#[test]
fn sides_match_logical_dtypes_even_when_empty() {
    for (left, right) in [
        (DataType::Int64, DataType::UInt64),
        (DataType::Int32, DataType::Date),
        (
            DataType::Datetime(TimeUnit::Milliseconds, None),
            DataType::Datetime(TimeUnit::Microseconds, None),
        ),
        (
            DataType::Datetime(TimeUnit::Nanoseconds, None),
            DataType::Datetime(
                TimeUnit::Nanoseconds,
                TimeZone::opt_try_new(Some("UTC")).unwrap(),
            ),
        ),
    ] {
        for (left_rows, right_rows) in
            [(&[][..], &[1][..]), (&[1][..], &[][..]), (&[][..], &[][..])]
        {
            let ls = typed_series(left_rows, &left);
            let rs = typed_series(right_rows, &right);
            for operation in OPERATIONS {
                let error = operation(&ls, &ls, &[], &rs, &rs, &[]).unwrap_err();
                assert!(matches!(error, PolarsError::InvalidOperation(_)));
                assert!(error.to_string().contains("across left and right"));
            }
        }
    }
}

#[test]
fn side_and_original_row_errors_precede_empty_and_unmatched_shortcuts() {
    let starts = Series::new("s".into(), [0i64, 1, 20, 30]);
    let ends = Series::new("e".into(), [0i64, 1, 20, 29]);
    let left_key = Series::new("key".into(), ["left-only"; 4]);
    let right_key = Series::new("key".into(), ["right-only"; 4]);
    for operation in OPERATIONS {
        for (ls, le, lk) in [
            (starts.clone(), starts.clone(), left_key.clone()),
            (starts.clear(), starts.clear(), left_key.clear()),
        ] {
            let error = operation(
                &ls,
                &le,
                &[lk],
                &starts,
                &ends,
                std::slice::from_ref(&right_key),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("right: interval at index 3"),
                "{error}"
            );
        }
        // As in every adapter, a side's nulls are rejected before its reversed rows.
        let null_ends = Series::new("e".into(), [Some(0i64), Some(0), None, Some(30)]);
        let error = operation(&starts, &null_ends, &[], &starts, &ends, &[]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("left does not support null endpoints"),
            "{error}"
        );
        let null_ends = Series::new("e".into(), [Some(0i64), None, Some(19), Some(30)]);
        let error = operation(&starts, &starts, &[], &starts, &null_ends, &[]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("right does not support null endpoints"),
            "{error}"
        );
    }
}

#[test]
fn keys_are_strict_and_never_cast_or_broadcast() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let ends = Series::new("e".into(), [2i64, 3]);
    for (left, right) in [
        (
            vec![Series::new("start".into(), [0i64, 1])],
            vec![Series::new("start".into(), [0i64, 1])],
        ),
        (vec![Series::new("key".into(), [0i64, 1])], vec![]),
        (
            vec![Series::new("key".into(), [0i64, 1])],
            vec![Series::new("other".into(), [0i64, 1])],
        ),
        (
            vec![Series::new("key".into(), [0i64, 1])],
            vec![Series::new("key".into(), [0i32, 1])],
        ),
        (
            vec![Series::new("key".into(), [0i64])],
            vec![Series::new("key".into(), [0i64, 1])],
        ),
        (
            vec![Series::new("key".into(), [0.0f64, 1.0])],
            vec![Series::new("key".into(), [0.0f64, 1.0])],
        ),
        (
            vec![Series::new("key".into(), [0i64, 1]); 2],
            vec![Series::new("key".into(), [0i64, 1]); 2],
        ),
    ] {
        for operation in OPERATIONS {
            assert!(operation(&starts, &ends, &left, &starts, &ends, &right).is_err());
        }
    }
}

#[test]
fn uint64_extremes_and_signed_extremes_need_no_coordinate_arithmetic() {
    let ls = Series::new("s".into(), [0u64, u64::MAX - 5]);
    let le = Series::new("e".into(), [1u64, u64::MAX]);
    let rs = Series::new("s".into(), [u64::MAX - 3]);
    let re = Series::new("e".into(), [u64::MAX - 1]);
    let result = subtract_intervals(&ls, &le, &[], &rs, &re, &[]).unwrap();
    assert_eq!(
        result["start"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [0, u64::MAX - 5, u64::MAX - 1]
    );
    assert_eq!(
        result["end"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [1, u64::MAX - 3, u64::MAX]
    );
    let ls = Series::new("s".into(), [i64::MIN]);
    let le = Series::new("e".into(), [i64::MAX]);
    let rs = Series::new("s".into(), [-1i64]);
    let re = Series::new("e".into(), [1i64]);
    assert_eq!(
        ranges(&subtract_intervals(&ls, &le, &[], &rs, &re, &[]).unwrap()),
        [(i64::MIN, -1), (1, i64::MAX)]
    );
}

fn cell_oracle(left: &[(i64, i64)], right: &[(i64, i64)], intersection: bool) -> Vec<(i64, i64)> {
    let mut boundaries = left
        .iter()
        .chain(right)
        .flat_map(|&(s, e)| [s, e])
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut output: Vec<(i64, i64)> = Vec::new();
    for cell in boundaries.windows(2) {
        let (start, end) = (cell[0], cell[1]);
        let a = left.iter().any(|&(s, e)| s <= start && start < e);
        let b = right.iter().any(|&(s, e)| s <= start && start < e);
        if a && b == intersection {
            if let Some(last) = output.last_mut().filter(|last| last.1 == start) {
                last.1 = end;
            } else {
                output.push((start, end));
            }
        }
    }
    output
}

proptest! {
    #[test]
    fn grouped_chunks_match_original_row_cell_oracle(
        left in prop::collection::vec((-12i64..=12, -12i64..=12, prop::option::of(0i64..4)), 0..28),
        right in prop::collection::vec((-12i64..=12, -12i64..=12, prop::option::of(0i64..5)), 0..28),
        cuts in prop::array::uniform6(0usize..29),
    ) {
        let make = |raw: &[(i64,i64,Option<i64>)]| (
            Series::new("start".into(), raw.iter().map(|r| r.0.min(r.1)).collect::<Vec<_>>()),
            Series::new("end".into(), raw.iter().map(|r| r.0.max(r.1)).collect::<Vec<_>>()),
            Series::new("key".into(), raw.iter().map(|r| r.2).collect::<Vec<_>>()),
        );
        let (ls,le,lk) = make(&left);
        let (rs,re,rk) = make(&right);
        let chunk = |series: &Series, cut: usize| {
            let cut = cut % (series.len() + 1);
            let mut result = series.slice(0,cut);
            result.append(&series.slice(cut as i64,series.len()-cut)).unwrap();
            result
        };
        let mut observed = Vec::new();
        for row in &left { if !observed.contains(&row.2) { observed.push(row.2); } }
        for (intersection, operation) in OPERATIONS.into_iter().enumerate() {
            let expected = observed.iter().flat_map(|key| {
                let select = |raw: &[(i64,i64,Option<i64>)]| raw.iter().filter(|r| r.2 == *key)
                    .map(|r| (r.0.min(r.1),r.0.max(r.1))).collect::<Vec<_>>();
                cell_oracle(&select(&left), &select(&right), intersection == 1).into_iter().map(|row| (*key,row))
            }).collect::<Vec<_>>();
            let result = operation(&ls,&le,std::slice::from_ref(&lk),&rs,&re,std::slice::from_ref(&rk)).unwrap();
            let actual = result["key"].i64().unwrap().iter().zip(ranges(&result)).collect::<Vec<_>>();
            prop_assert_eq!(actual,expected);
            let chunked = operation(&chunk(&ls,cuts[0]),&chunk(&le,cuts[1]),&[chunk(&lk,cuts[2])],
                &chunk(&rs,cuts[3]),&chunk(&re,cuts[4]),&[chunk(&rk,cuts[5])]).unwrap();
            prop_assert!(result.equals_missing(&chunked));
        }
    }
}
