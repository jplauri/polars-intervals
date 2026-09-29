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
