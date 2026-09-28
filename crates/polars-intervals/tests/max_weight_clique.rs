use polars::prelude::*;
use polars_intervals::max_weight_clique as solve;

mod support;
use support::{INTEGER_DTYPES, endpoint_dtypes, typed_series};

fn values(mask: Series) -> Vec<bool> {
    assert_eq!(mask.dtype(), &DataType::Boolean);
    assert_eq!(mask.null_count(), 0);
    assert_eq!(mask.name().as_str(), "max_weight_clique");
    mask.bool().unwrap().no_null_iter().collect()
}

#[test]
fn all_supported_dtypes_preserve_chunks_units_and_original_order() {
    for endpoint in endpoint_dtypes() {
        let convert = |rows: &[i64]| typed_series(rows, &endpoint);
        let mut starts = convert(&[10, 0]);
        starts.append(&convert(&[1, 2, 7, 7])).unwrap();
        let mut ends = convert(&[11]);
        ends.append(&convert(&[5, 4, 3, 7, 7])).unwrap();
        assert_eq!(
            values(solve(&starts, &ends, None).unwrap()),
            [false, true, true, true, false, false]
        );
        for dtype in INTEGER_DTYPES {
            let mut weights = Series::new("w".into(), [9i64, 1, 1]).cast(&dtype).unwrap();
            weights
                .append(&Series::new("w".into(), [1i64, 8, 8]).cast(&dtype).unwrap())
                .unwrap();
            assert_eq!(
                values(solve(&starts, &ends, Some(&weights)).unwrap()),
                [true, false, false, false, false, false]
            );
            let ones = Series::new("ones".into(), [1i64; 6]).cast(&dtype).unwrap();
            assert_eq!(
                values(solve(&starts, &ends, Some(&ones)).unwrap()),
                values(solve(&starts, &ends, None).unwrap())
            );
            for explicit in [None, Some(weights.slice(0, 0))] {
                assert!(
                    values(
                        solve(&starts.slice(0, 0), &ends.slice(0, 0), explicit.as_ref()).unwrap()
                    )
                    .is_empty()
                );
            }
        }
    }
}

#[test]
fn exact_integer_and_temporal_boundaries() {
    for endpoint in [
        DataType::Int64,
        DataType::Date,
        DataType::Datetime(TimeUnit::Milliseconds, None),
        DataType::Datetime(TimeUnit::Microseconds, None),
        DataType::Datetime(TimeUnit::Nanoseconds, None),
    ] {
        let (minimum, maximum) = if endpoint == DataType::Date {
            (i64::from(i32::MIN), i64::from(i32::MAX))
        } else {
            (i64::MIN, i64::MAX)
        };
        let starts = typed_series(&[minimum, maximum - 2, maximum - 1], &endpoint);
        let ends = typed_series(&[maximum - 2, maximum - 1, maximum], &endpoint);
        let weights = Series::new("w".into(), [u64::MAX - 1, u64::MAX, u64::MAX - 1]);
        assert_eq!(
            values(solve(&starts, &ends, Some(&weights)).unwrap()),
            [false, true, false]
        );
    }
    let starts = Series::new("s".into(), [u64::MAX - 2, u64::MAX - 1, u64::MAX]);
    let ends = Series::new("e".into(), [u64::MAX, u64::MAX, u64::MAX]);
    let weights = Series::new("w".into(), [u64::MAX, u64::MAX, u64::MAX]);
    assert_eq!(
        values(solve(&starts, &ends, Some(&weights)).unwrap()),
        [true, true, false]
    );
}

#[test]
fn signed_weights_and_isolated_empty_vertices() {
    let starts = Series::new("s".into(), [0i64, 1, 2, 1, 1]);
    let ends = Series::new("e".into(), [5i64, 4, 3, 1, 1]);
    for (weights, expected) in [
        (
            [10i64, i64::MIN, 9, 19, 19],
            [true, false, true, false, false],
        ),
        ([10, -100, 9, 20, 20], [false, false, false, true, false]),
        ([-1, 0, -3, 0, -1], [false; 5]),
    ] {
        assert_eq!(
            values(solve(&starts, &ends, Some(&Series::new("w".into(), weights))).unwrap()),
            expected
        );
    }
    let empty = Series::new("s".into(), [5i64, 5, 2]);
    assert_eq!(
        values(solve(&empty, &empty, None).unwrap()),
        [true, false, false]
    );
}

#[test]
fn every_row_is_validated_before_fast_paths_with_original_error_indices() {
    let starts = Series::new("s".into(), [0i64, 5, 2]);
    let ends = Series::new("e".into(), [0i64, 5, 1]);
    for weights in [None, Some(Series::new("w".into(), [0i64, -2, -3]))] {
        let error = solve(&starts, &ends, weights.as_ref())
            .unwrap_err()
            .to_string();
        assert!(error.contains("index 2"), "{error}");
    }
    let mut chunked = starts.slice(0, 2);
    chunked.append(&starts.slice(2, 1)).unwrap();
    assert!(
        solve(&chunked, &ends, None)
            .unwrap_err()
            .to_string()
            .contains("index 2")
    );
}

#[test]
fn unequal_lengths_nulls_and_unsupported_dtypes_are_errors() {
    let starts = Series::new("s".into(), [0i64, 1, 2]);
    let ends = Series::new("e".into(), [2i64, 3, 4]);
    let weights = Series::new("w".into(), [1i64, 0, -1]);
    for (s, e, w, message) in [
        (
            starts.clone(),
            ends.clone(),
            Some(weights.slice(0, 1)),
            "equal lengths",
        ),
        (starts.slice(0, 1), ends.clone(), None, "equal lengths"),
        (
            starts.clone(),
            ends.slice(0, 1),
            Some(weights.clone()),
            "equal lengths",
        ),
        (
            starts.clone(),
            ends.cast(&DataType::Int32).unwrap(),
            None,
            "matching integer",
        ),
        (
            Series::full_null("s".into(), 3, &DataType::Int64),
            ends.clone(),
            None,
            "null endpoints",
        ),
        (
            starts.clone(),
            Series::full_null("e".into(), 3, &DataType::Int64),
            Some(weights.clone()),
            "null endpoints",
        ),
        (
            starts.clone(),
            ends.clone(),
            Some(Series::full_null("w".into(), 3, &DataType::Int64)),
            "null weights",
        ),
    ] {
        let error = solve(&s, &e, w.as_ref()).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
    for dtype in [
        DataType::Float32,
        DataType::Float64,
        DataType::Boolean,
        DataType::String,
        DataType::Int128,
        DataType::Decimal(20, 2),
        DataType::Date,
        DataType::Datetime(TimeUnit::Microseconds, None),
        DataType::Duration(TimeUnit::Microseconds),
        DataType::Time,
        DataType::Null,
    ] {
        for length in [0, 3] {
            let unsupported = Series::full_null("x".into(), length, &dtype);
            let error = solve(
                &starts.slice(0, length),
                &ends.slice(0, length),
                Some(&unsupported),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("integer weight dtype"), "{error}");
            if !matches!(dtype, DataType::Date | DataType::Datetime(_, _)) {
                let error = solve(&unsupported, &unsupported, None)
                    .unwrap_err()
                    .to_string();
                assert!(
                    error.contains("integer dtype, Date, or Datetime"),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn logical_temporal_metadata_must_match_even_when_empty() {
    let utc = TimeZone::opt_try_new(Some("UTC")).unwrap();
    let helsinki = TimeZone::opt_try_new(Some("Europe/Helsinki")).unwrap();
    for (left, right) in [
        (DataType::Date, DataType::Int32),
        (
            DataType::Datetime(TimeUnit::Milliseconds, None),
            DataType::Datetime(TimeUnit::Microseconds, None),
        ),
        (
            DataType::Datetime(TimeUnit::Nanoseconds, utc.clone()),
            DataType::Datetime(TimeUnit::Nanoseconds, helsinki),
        ),
        (
            DataType::Datetime(TimeUnit::Nanoseconds, utc),
            DataType::Datetime(TimeUnit::Nanoseconds, None),
        ),
    ] {
        for data in [&[][..], &[0i64][..]] {
            let error = solve(
                &typed_series(data, &left),
                &typed_series(data, &right),
                None,
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("matching integer, Date, or Datetime"),
                "{error}"
            );
        }
    }
}
