use polars::prelude::*;
use polars_intervals::max_k_coverage;

mod support;
use support::{endpoint_dtypes, typed_series};

#[test]
fn endpoint_dtypes_chunks_and_row_order() {
    for dtype in endpoint_dtypes() {
        let convert = |values: &[i64]| typed_series(values, &dtype);
        let mut s = convert(&[10]);
        s.append(&convert(&[0, 1, 8])).unwrap();
        let mut e = convert(&[18, 10]);
        e.append(&convert(&[11, 8])).unwrap();
        let result = max_k_coverage(&s, &e, 2).unwrap();
        assert_eq!(result.dtype(), &DataType::Boolean);
        assert_eq!(result.null_count(), 0);
        assert_eq!(
            result.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
            [true, true, false, false]
        );
        assert!(
            max_k_coverage(&s.slice(0, 0), &e.slice(0, 0), 100)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            max_k_coverage(&s, &e, 0).unwrap().bool().unwrap().sum(),
            Some(0)
        );
    }
}

#[test]
fn validation_even_for_zero_budget() {
    let s = Series::new("s".into(), [0i64, 4]);
    let e = Series::new("e".into(), [0i64, 3]);
    for k in [0, 1, 100] {
        assert!(
            max_k_coverage(&s, &e, k)
                .unwrap_err()
                .to_string()
                .contains("index 1")
        );
        assert!(
            max_k_coverage(&s, &e.slice(0, 1), k)
                .unwrap_err()
                .to_string()
                .contains("equal lengths")
        );
        assert!(
            max_k_coverage(&s, &e.cast(&DataType::Int32).unwrap(), k)
                .unwrap_err()
                .to_string()
                .contains("matching")
        );
        assert!(
            max_k_coverage(&Series::full_null("s".into(), 2, &DataType::Int64), &e, k)
                .unwrap_err()
                .to_string()
                .contains("null endpoints")
        );
    }
}
