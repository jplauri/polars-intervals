use polars::prelude::*;
use polars_intervals::containment_count;

mod support;
use support::{endpoint_dtypes, typed_series};

fn check(s: &Series, e: &Series, expected: &[u64]) {
    let result = containment_count(s, e).unwrap();
    assert_eq!(result.name().as_str(), "containment_count");
    assert_eq!(result.dtype(), &DataType::UInt64);
    assert_eq!(result.null_count(), 0);
    assert_eq!(
        result
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn supported_integer_and_temporal_types_and_empty_input() {
    for dtype in endpoint_dtypes() {
        let s = typed_series(&[0, 1, 1, 5], &dtype);
        let e = typed_series(&[10, 5, 5, 5], &dtype);
        assert_eq!(s.dtype(), &dtype);
        check(&s, &e, &[3, 2, 2, 0]);
        check(&s.slice(0, 0), &e.slice(0, 0), &[]);
    }
}

#[test]
fn counts_across_misaligned_chunks_in_original_order() {
    let mut s = Series::new("s".into(), [2i64, 0]);
    s.append(&Series::new("s".into(), [3i64, 1])).unwrap();
    let mut e = Series::new("e".into(), [8i64]);
    e.append(&Series::new("e".into(), [10i64, 7, 9])).unwrap();
    check(&s, &e, &[1, 3, 0, 2]);
}

#[test]
fn counts_can_exceed_endpoint_width() {
    check(
        &Series::new("s".into(), vec![0u8; 300]),
        &Series::new("e".into(), vec![0u8; 300]),
        &[299; 300],
    );
}

#[test]
fn preserves_signed_and_unsigned_extremes() {
    check(
        &Series::new("s".into(), [i64::MIN, i64::MAX - 1, i64::MAX]),
        &Series::new("e".into(), [i64::MAX; 3]),
        &[2, 1, 0],
    );
    check(
        &Series::new("s".into(), [0u64, u64::MAX - 1, u64::MAX]),
        &Series::new("e".into(), [u64::MAX; 3]),
        &[2, 1, 0],
    );
}
