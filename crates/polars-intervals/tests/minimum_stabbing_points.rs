use polars::prelude::*;
use polars_intervals::minimum_stabbing_points;

mod support;
use support::{endpoint_dtypes, typed_series};

#[test]
fn logical_list_dtype_chunks_and_empty() {
    for dtype in endpoint_dtypes() {
        let convert = |values: &[i64]| typed_series(values, &dtype);
        let mut s = convert(&[5]);
        s.append(&convert(&[0, 2])).unwrap();
        let mut e = convert(&[9, 4]);
        e.append(&convert(&[6])).unwrap();
        let result = minimum_stabbing_points(&s, &e).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.null_count(), 0);
        assert_eq!(result.dtype(), &DataType::List(Box::new(dtype.clone())));
        let points = result.list().unwrap().get_as_series(0).unwrap();
        // ListChunked::get_as_series exposes the physical child in Polars 0.55.
        assert_eq!(points.dtype(), &dtype.to_physical());
        assert_eq!(
            points
                .cast(&DataType::Int64)
                .unwrap()
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [3, 8]
        );
        let empty = minimum_stabbing_points(&s.slice(0, 0), &e.slice(0, 0)).unwrap();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty.dtype(), result.dtype());
        assert!(empty.list().unwrap().get_as_series(0).unwrap().is_empty());
    }
}

#[test]
fn empty_interval_reports_original_row() {
    let s = Series::new("s".into(), [6i64, 3, 0]);
    let e = Series::new("e".into(), [9i64, 3, 2]);
    assert!(
        minimum_stabbing_points(&s, &e)
            .unwrap_err()
            .to_string()
            .contains("cannot stab empty interval at index 1")
    );
}
