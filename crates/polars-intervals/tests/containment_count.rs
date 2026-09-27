use polars::prelude::*;
use polars_intervals::containment_count;

fn with_dtype(values: &Series, dtype: &DataType) -> Series {
    match dtype {
        DataType::Datetime(unit, zone) => values
            .i64()
            .unwrap()
            .clone()
            .into_datetime(*unit, zone.clone())
            .into_series(),
        _ => values.cast(dtype).unwrap(),
    }
}

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
    let s = Series::new("s".into(), [0i64, 1, 1, 5]);
    let e = Series::new("e".into(), [10i64, 5, 5, 5]);
    for dtype in [
        DataType::Int8,
        DataType::Int16,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt8,
        DataType::UInt16,
        DataType::UInt32,
        DataType::UInt64,
        DataType::Date,
        DataType::Datetime(TimeUnit::Milliseconds, None),
        DataType::Datetime(TimeUnit::Microseconds, None),
        DataType::Datetime(TimeUnit::Nanoseconds, None),
        DataType::Datetime(TimeUnit::Nanoseconds, Some(TimeZone::UTC)),
    ] {
        let s = with_dtype(&s, &dtype);
        let e = with_dtype(&e, &dtype);
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

#[test]
fn mismatched_logical_types_never_coerce() {
    let s = Series::new("s".into(), [0i64]);
    for (a, b) in [
        (DataType::Int64, DataType::UInt64),
        (DataType::Date, DataType::Int32),
        (
            DataType::Datetime(TimeUnit::Milliseconds, None),
            DataType::Datetime(TimeUnit::Microseconds, None),
        ),
        (
            DataType::Datetime(TimeUnit::Microseconds, None),
            DataType::Datetime(TimeUnit::Microseconds, Some(TimeZone::UTC)),
        ),
    ] {
        assert!(matches!(
            containment_count(&with_dtype(&s, &a), &with_dtype(&s, &b)),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
}

#[test]
fn rejects_nulls_unsupported_types_lengths_and_first_invalid_row() {
    let s = Series::new("s".into(), [0i64, 1]);
    let nulls = Series::new("e".into(), [Some(2i64), None]);
    for (a, b) in [(&s, &nulls), (&nulls, &s), (&nulls, &nulls)] {
        assert!(matches!(
            containment_count(a, b),
            Err(PolarsError::ComputeError(_))
        ));
    }
    for (a, b) in [(&s, &s.slice(0, 1)), (&s.slice(0, 0), &s)] {
        assert!(matches!(
            containment_count(a, b),
            Err(PolarsError::ShapeMismatch(_))
        ));
    }
    for dtype in [
        DataType::Float64,
        DataType::Int128,
        DataType::Boolean,
        DataType::String,
        DataType::Duration(TimeUnit::Microseconds),
        DataType::Time,
    ] {
        let empty = Series::new_empty("s".into(), &dtype);
        assert!(matches!(
            containment_count(&empty, &empty),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
    let err = containment_count(
        &Series::new("s".into(), [0i64, 3, 4]),
        &Series::new("e".into(), [0i64, 2, 1]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("index 1"));
}
