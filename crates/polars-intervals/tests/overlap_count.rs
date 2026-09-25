use polars::prelude::*;
use polars_intervals::overlap_count;

fn assert_counts(starts: &Series, ends: &Series, expected: &[u64]) {
    let counts = overlap_count(starts, ends).unwrap();
    assert_eq!(counts.name().as_str(), "overlap_count");
    assert_eq!(counts.dtype(), &DataType::UInt64);
    assert_eq!(counts.len(), starts.len());
    assert_eq!(counts.null_count(), 0);
    assert_eq!(
        counts
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn all_supported_integer_types_match_core() {
    // Unsorted inputs include nesting, duplicates, disjoint intervals and empties.
    let raw_starts = [3i64, 0, 1, 1, 4, 1, 8];
    let raw_ends = [6i64, 5, 2, 2, 4, 1, 9];
    let expected: Vec<u64> = intervals_core::overlap_counts(&raw_starts, &raw_ends)
        .unwrap()
        .into_iter()
        .map(|count| count as u64)
        .collect();
    assert_eq!(expected, [1, 3, 2, 2, 0, 0, 0]);

    for dtype in [
        DataType::Int8,
        DataType::Int16,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt8,
        DataType::UInt16,
        DataType::UInt32,
        DataType::UInt64,
    ] {
        let starts = Series::new("starts".into(), raw_starts)
            .cast(&dtype)
            .unwrap();
        let ends = Series::new("ends".into(), raw_ends).cast(&dtype).unwrap();
        assert_counts(&starts, &ends, &expected);
        assert_counts(&starts.slice(1, 4), &ends.slice(1, 4), &[2, 2, 2, 0]);
        assert_counts(&starts.slice(0, 0), &ends.slice(0, 0), &[]);
    }
}

#[test]
fn preserves_order_across_different_chunk_boundaries_and_slices() {
    let mut starts = Series::new("starts".into(), [99i64, 3, 0]);
    starts
        .append(&Series::new("starts".into(), [1i64, 1, 4, 1, 8, 99]))
        .unwrap();
    let mut ends = Series::new("ends".into(), [99i64, 6]);
    ends.append(&Series::new("ends".into(), [5i64, 2, 2]))
        .unwrap();
    ends.append(&Series::new("ends".into(), [4i64, 1, 9, 99]))
        .unwrap();
    let starts = starts.slice(1, 7);
    let ends = ends.slice(1, 7);
    assert_eq!(starts.n_chunks(), 2);
    assert_eq!(ends.n_chunks(), 3);
    for starts in [&starts, &starts.rechunk()] {
        for ends in [&ends, &ends.rechunk()] {
            assert_counts(starts, ends, &[1, 3, 2, 2, 0, 0, 0]);
        }
    }
}

#[test]
fn touching_intervals_do_not_overlap() {
    assert_counts(
        &Series::new("starts".into(), [1i32, 3, 5]),
        &Series::new("ends".into(), [3i32, 5, 7]),
        &[0, 0, 0],
    );
    assert_counts(
        &Series::new("starts".into(), [1i32, 3]),
        &Series::new("ends".into(), [4i32, 5]),
        &[1, 1],
    );
}

#[test]
fn single_intervals_exclude_themselves() {
    let starts = Series::new("starts".into(), [1i32]);
    for end in [1i32, 3] {
        assert_counts(&starts, &Series::new("ends".into(), [end]), &[0]);
    }
}

#[test]
fn counts_are_not_limited_to_the_endpoint_width() {
    let starts = Series::new("starts".into(), vec![0u8; 300]);
    let ends = Series::new("ends".into(), vec![1u8; 300]);
    assert_counts(&starts, &ends, &vec![299; 300]);
}

#[test]
fn signed_extremes_are_preserved() {
    let starts = Series::new("starts".into(), [i64::MIN, i64::MIN, 0, i64::MAX]);
    let ends = Series::new("ends".into(), [i64::MAX, 0, i64::MAX, i64::MAX]);
    assert_counts(&starts, &ends, &[2, 1, 1, 0]);
}

#[test]
fn unsigned_values_above_i64_max_remain_distinct() {
    let starts = Series::new(
        "starts".into(),
        [0u64, u64::MAX - 2, u64::MAX - 1, u64::MAX],
    );
    let ends = Series::new("ends".into(), [u64::MAX; 4]);
    assert_counts(&starts, &ends, &[2, 2, 2, 0]);
}

#[test]
fn rejects_nulls_in_either_input_including_all_null_inputs() {
    let values = Series::new("values".into(), [0i64, 2]);
    for nulls in [
        Series::new("nulls".into(), [Some(0i64), None]),
        Series::full_null("nulls".into(), 2, &DataType::Int64),
    ] {
        for (starts, ends) in [(&nulls, &values), (&values, &nulls), (&nulls, &nulls)] {
            assert!(matches!(
                overlap_count(starts, ends),
                Err(PolarsError::ComputeError(_))
            ));
        }
    }
}

#[test]
fn rejects_mixed_integer_dtypes() {
    let starts = Series::new("starts".into(), [0i32, 1]);
    for ends in [
        Series::new("ends".into(), [2i64, 3]),
        Series::new("ends".into(), [2u32, 3]),
    ] {
        assert!(matches!(
            overlap_count(&starts, &ends),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
}

#[test]
fn rejects_unsupported_dtypes_even_when_empty() {
    for values in [
        Series::new("values".into(), [0.0f32, 1.0]),
        Series::new("values".into(), [0.0f64, 1.0]),
        Series::new("values".into(), [false, true]),
        Series::new("values".into(), ["a", "b"]),
        Series::full_null("values".into(), 2, &DataType::Null),
    ] {
        for input in [&values, &values.slice(0, 0)] {
            assert!(matches!(
                overlap_count(input, input),
                Err(PolarsError::InvalidOperation(_))
            ));
        }
    }
}

#[test]
fn rejects_length_mismatches_without_broadcasting() {
    let empty = Series::new("empty".into(), Vec::<i64>::new());
    let one = Series::new("one".into(), [0i64]);
    let two = Series::new("two".into(), [1i64, 2]);
    for (starts, ends) in [(&empty, &one), (&one, &empty), (&one, &two), (&two, &one)] {
        assert!(matches!(
            overlap_count(starts, ends),
            Err(PolarsError::ShapeMismatch(_))
        ));
    }
}

#[test]
fn invalid_interval_error_identifies_the_first_original_row() {
    let starts = Series::new("starts".into(), [3i64, 0, 9, 7]);
    let ends = Series::new("ends".into(), [5i64, 2, 8, 6]);
    let error = overlap_count(&starts, &ends).unwrap_err();
    assert!(matches!(error, PolarsError::ComputeError(_)));
    assert!(error.to_string().contains("index 2"));
}

#[test]
fn rejects_reversed_intervals_at_integer_extremes() {
    for (starts, ends) in [
        (
            Series::new("starts".into(), [i64::MIN + 1]),
            Series::new("ends".into(), [i64::MIN]),
        ),
        (
            Series::new("starts".into(), [u64::MAX]),
            Series::new("ends".into(), [u64::MAX - 1]),
        ),
    ] {
        assert!(matches!(
            overlap_count(&starts, &ends),
            Err(PolarsError::ComputeError(_))
        ));
    }
}
