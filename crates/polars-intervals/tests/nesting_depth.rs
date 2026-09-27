use polars::prelude::*;
use polars_intervals::{containment_count, nesting_depth};

fn with_dtype(values: &Series, dtype: &DataType) -> Series {
    match dtype {
        DataType::Datetime(unit, zone) => values.clone().into_datetime(*unit, zone.clone()),
        _ => values.cast(dtype).unwrap(),
    }
}

fn check(starts: &Series, ends: &Series, expected: &[u64]) {
    let result = nesting_depth(starts, ends).unwrap();
    assert_eq!(result.name().as_str(), "nesting_depth");
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
    let starts = Series::new("s".into(), [0i64, 1, 1, 5]);
    let ends = Series::new("e".into(), [10i64, 5, 5, 5]);
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
        DataType::Datetime(TimeUnit::Milliseconds, Some(TimeZone::UTC)),
        DataType::Datetime(TimeUnit::Microseconds, Some(TimeZone::UTC)),
        DataType::Datetime(TimeUnit::Nanoseconds, Some(TimeZone::UTC)),
    ] {
        let starts = with_dtype(&starts, &dtype);
        let ends = with_dtype(&ends, &dtype);
        assert_eq!(starts.dtype(), &dtype);
        check(&starts, &ends, &[0, 1, 1, 2]);
        check(&starts.slice(0, 0), &ends.slice(0, 0), &[]);
    }
}

#[test]
fn depths_across_misaligned_chunks_and_slices_keep_original_order() {
    let mut starts = Series::new("s".into(), [99i64, 2, 0]);
    starts
        .append(&Series::new("s".into(), [3i64, 1, 99]))
        .unwrap();
    let mut ends = Series::new("e".into(), [99i64, 8]);
    ends.append(&Series::new("e".into(), [10i64])).unwrap();
    ends.append(&Series::new("e".into(), [7i64, 9, 99]))
        .unwrap();
    check(&starts.slice(1, 4), &ends.slice(1, 4), &[2, 0, 3, 1]);
}

#[test]
fn exact_duplicate_batching_preserves_equal_start_and_equal_end_chains() {
    for (starts, ends) in [([0i64, 0, 0, 0], [10i64, 8, 8, 5]), ([0, 2, 2, 5], [10; 4])] {
        check(
            &Series::new("s".into(), starts),
            &Series::new("e".into(), ends),
            &[0, 1, 1, 2],
        );
    }
}

#[test]
fn empty_endpoint_semantics_include_right_boundary_and_exclude_identical_empties() {
    check(
        &Series::new("s".into(), [0i64, 2, 5, 5]),
        &Series::new("e".into(), [10i64, 5, 5, 5]),
        &[0, 1, 2, 2],
    );
    check(
        &Series::new("s".into(), [3i64; 3]),
        &Series::new("e".into(), [3i64; 3]),
        &[0; 3],
    );
}

#[test]
fn duplicate_multiplicity_does_not_increase_depth() {
    check(
        &Series::new("s".into(), vec![0u8; 300]),
        &Series::new("e".into(), vec![0u8; 300]),
        &[0; 300],
    );
}

#[test]
fn preserves_signed_and_unsigned_extremes() {
    check(
        &Series::new("s".into(), [i64::MIN, i64::MAX - 1, i64::MAX]),
        &Series::new("e".into(), [i64::MAX; 3]),
        &[0, 1, 2],
    );
    check(
        &Series::new("s".into(), [0u64, u64::MAX - 1, u64::MAX]),
        &Series::new("e".into(), [u64::MAX; 3]),
        &[0, 1, 2],
    );
}

#[test]
fn mismatched_logical_types_never_coerce() {
    let values = Series::new("s".into(), [0i64]);
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
            nesting_depth(&with_dtype(&values, &a), &with_dtype(&values, &b)),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
}

#[test]
fn rejects_nulls_unsupported_types_lengths_and_first_invalid_row() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let nulls = Series::new("e".into(), [Some(2i64), None]);
    for (a, b) in [(&starts, &nulls), (&nulls, &starts), (&nulls, &nulls)] {
        assert!(matches!(
            nesting_depth(a, b),
            Err(PolarsError::ComputeError(_))
        ));
    }
    for (a, b) in [
        (&starts, &starts.slice(0, 1)),
        (&starts.slice(0, 0), &starts),
    ] {
        assert!(matches!(
            nesting_depth(a, b),
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
            nesting_depth(&empty, &empty),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
    let err = nesting_depth(
        &Series::new("s".into(), [0i64, 3, 4]),
        &Series::new("e".into(), [0i64, 2, 1]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("index 1"));
}

#[test]
fn incomparable_containers_demonstrate_depth_is_not_a_containment_count() {
    let starts = Series::new("s".into(), [0i64, 1, 2, 4, 4]);
    let ends = Series::new("e".into(), [8i64, 9, 10, 5, 5]);
    check(&starts, &ends, &[0, 0, 0, 1, 1]);
    assert_eq!(
        containment_count(&starts, &ends)
            .unwrap()
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [2, 2, 2, 1, 1],
    );
}
