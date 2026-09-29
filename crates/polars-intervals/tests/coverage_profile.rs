use polars::prelude::*;
use polars_intervals::coverage_profile;

mod support;
use support::{INTEGER_DTYPES, endpoint_dtypes, typed_series};

fn values(frame: &DataFrame) -> Vec<(i64, i64, i128)> {
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
        .zip(frame["load"].i128().unwrap().into_no_null_iter())
        .map(|((start, end), load)| (start, end, load))
        .collect()
}

#[test]
fn exact_typed_output_for_every_endpoint_and_weight_dtype() {
    for dtype in endpoint_dtypes() {
        let starts = typed_series(&[0, 2, 5], &dtype);
        let ends = typed_series(&[4, 5, 7], &dtype);
        let units = coverage_profile(&starts, &ends, None, &[], None, false).unwrap();
        assert_eq!(units.get_column_names(), ["start", "end", "load"]);
        assert_eq!(units["start"].dtype(), &dtype);
        assert_eq!(units["end"].dtype(), &dtype);
        assert_eq!(units["load"].dtype(), &DataType::Int128);
        assert_eq!(units["load"].null_count(), 0);
        assert_eq!(values(&units), [(0, 2, 1), (2, 4, 2), (4, 7, 1)]);
        for weight_dtype in INTEGER_DTYPES {
            let weights = typed_series(&[2, 3, 3], &weight_dtype);
            let result = coverage_profile(&starts, &ends, Some(&weights), &[], None, true).unwrap();
            assert_eq!(values(&result), [(0, 2, 2), (2, 4, 5), (4, 7, 3)]);
        }
        let result =
            coverage_profile(&starts.clear(), &ends.clear(), None, &[], None, true).unwrap();
        assert_eq!(result.height(), 0);
        assert_eq!(result["start"].dtype(), &dtype);
        assert_eq!(result["end"].dtype(), &dtype);
        assert_eq!(result["load"].dtype(), &DataType::Int128);
    }
}

#[test]
fn uint64_loads_and_endpoints_are_exact_beyond_float_and_uint64() {
    let x = u64::MAX - 4;
    let starts = Series::new("s".into(), [x, x + 1]);
    let ends = Series::new("e".into(), [x + 3, x + 4]);
    let weights = Series::new("w".into(), [u64::MAX, u64::MAX]);
    let result = coverage_profile(&starts, &ends, Some(&weights), &[], None, false).unwrap();
    assert_eq!(
        result["start"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [x, x + 1, x + 3]
    );
    assert_eq!(
        result["end"]
            .u64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [x + 1, x + 3, x + 4]
    );
    assert_eq!(
        result["load"]
            .i128()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        [
            i128::from(u64::MAX),
            2 * i128::from(u64::MAX),
            i128::from(u64::MAX)
        ]
    );
}

#[test]
fn scalar_domains_and_zero_weight_hulls_preserve_temporal_metadata() {
    for dtype in endpoint_dtypes() {
        let starts = typed_series(&[0, 10, 120], &dtype);
        let ends = typed_series(&[100, 20, 120], &dtype);
        let weights = Series::new("w".into(), [0i64, 7, 9]);
        let full = coverage_profile(&starts, &ends, Some(&weights), &[], None, true).unwrap();
        assert_eq!(values(&full), [(0, 10, 0), (10, 20, 7), (20, 100, 0)]);
        let left = Scalar::new(
            dtype.clone(),
            typed_series(&[15], &dtype).get(0).unwrap().into_static(),
        );
        let right = Scalar::new(
            dtype.clone(),
            typed_series(&[25], &dtype).get(0).unwrap().into_static(),
        );
        let result = coverage_profile(
            &starts,
            &ends,
            Some(&weights),
            &[],
            Some((&left, &right)),
            true,
        )
        .unwrap();
        assert_eq!(values(&result), [(15, 20, 7), (20, 25, 0)]);
        assert_eq!(result["start"].dtype(), &dtype);
        let empty = coverage_profile(
            &starts.clear(),
            &ends.clear(),
            None,
            &[],
            Some((&left, &right)),
            true,
        )
        .unwrap();
        assert_eq!(values(&empty), [(15, 25, 0)]);
        assert_eq!(empty["end"].dtype(), &dtype);
    }
}

#[test]
fn stable_native_groups_keep_null_keys_and_all_chunks() {
    let mut starts = Series::new("s".into(), [0i64, 1]);
    starts.append(&Series::new("s".into(), [2i64, 3])).unwrap();
    let mut ends = Series::new("e".into(), [5i64]);
    ends.append(&Series::new("e".into(), [6i64, 7, 8])).unwrap();
    let mut keys = Series::new("resource".into(), [Some("b"), None, Some("b")]);
    keys.append(&Series::new("resource".into(), [None::<&str>]))
        .unwrap();
    let flags = Series::new("enabled".into(), [Some(true), None, Some(true), None]);
    let result =
        coverage_profile(&starts, &ends, None, &[keys.clone(), flags], None, false).unwrap();
    assert_eq!(
        result.get_column_names(),
        ["resource", "enabled", "start", "end", "load"]
    );
    assert_eq!(
        values(&result),
        [
            (0, 2, 1),
            (2, 5, 2),
            (5, 7, 1),
            (1, 3, 1),
            (3, 6, 2),
            (6, 8, 1)
        ]
    );
    assert_eq!(
        result["resource"].str().unwrap().iter().collect::<Vec<_>>(),
        [Some("b"), Some("b"), Some("b"), None, None, None]
    );
    assert_eq!(result["enabled"].dtype(), &DataType::Boolean);
    let rechunked = starts.rechunk();
    let one_key = coverage_profile(
        &starts,
        &ends,
        None,
        std::slice::from_ref(&keys),
        None,
        false,
    )
    .unwrap();
    assert!(
        one_key.equals_missing(
            &coverage_profile(
                &rechunked,
                &ends.rechunk(),
                None,
                &[keys.rechunk()],
                None,
                false
            )
            .unwrap()
        )
    );
}

#[test]
fn every_supported_key_dtype_survives_zero_output_and_empty_groups() {
    let starts = Series::new("s".into(), [0i64, 2]);
    let ends = starts.clone();
    let left = Scalar::from(0i64);
    let right = Scalar::from(5i64);
    for dtype in endpoint_dtypes() {
        let mut key = typed_series(&[1, 2], &dtype);
        key.rename("key".into());
        let zero = coverage_profile(
            &starts,
            &ends,
            None,
            std::slice::from_ref(&key),
            None,
            false,
        )
        .unwrap();
        assert_eq!(zero.height(), 0);
        assert_eq!(zero["key"].dtype(), &dtype);
        let full = coverage_profile(
            &starts,
            &ends,
            None,
            std::slice::from_ref(&key),
            Some((&left, &right)),
            true,
        )
        .unwrap();
        assert_eq!(values(&full), [(0, 5, 0), (0, 5, 0)]);
        assert_eq!(full["key"].dtype(), &dtype);
        let empty = coverage_profile(
            &starts.clear(),
            &ends.clear(),
            None,
            &[key.clear()],
            Some((&left, &right)),
            true,
        )
        .unwrap();
        assert_eq!(empty.height(), 0);
        assert_eq!(empty["key"].dtype(), &dtype);
    }
}

#[test]
fn invalid_lengths_keys_and_bounds_fail_without_panicking() {
    let starts = Series::new("s".into(), [0i64, 2]);
    let ends = Series::new("e".into(), [2i64, 4]);
    let weights = Series::new("w".into(), [1i64]);
    assert!(matches!(
        coverage_profile(&starts, &ends, Some(&weights), &[], None, false),
        Err(PolarsError::ShapeMismatch(_))
    ));
    assert!(matches!(
        coverage_profile(&starts, &ends, None, &[weights], None, false),
        Err(PolarsError::ShapeMismatch(_))
    ));
    for keys in [
        vec![Series::new("start".into(), [1i64, 2])],
        vec![
            Series::new("k".into(), [1i64, 2]),
            Series::new("k".into(), [1i64, 2]),
        ],
        vec![Series::new("k".into(), [1f64, 2.0])],
    ] {
        assert!(matches!(
            coverage_profile(&starts, &ends, None, &keys, None, false),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
    let left = Scalar::from(0i32);
    let right = Scalar::from(3i64);
    assert!(matches!(
        coverage_profile(&starts, &ends, None, &[], Some((&left, &right)), true),
        Err(PolarsError::InvalidOperation(_))
    ));
    let left = Scalar::from(4i64);
    assert!(
        coverage_profile(&starts, &ends, None, &[], Some((&left, &right)), true)
            .unwrap_err()
            .to_string()
            .contains("domain")
    );
}

#[test]
fn validates_all_original_rows_before_grouping_clipping_or_empty_output() {
    let starts = Series::new("s".into(), [0i64, 1, 200, 300]);
    let ends = Series::new("e".into(), [0i64, 1, 200, 299]);
    let keys = [Series::new("key".into(), ["a", "b", "b", "a"])];
    let bound = Scalar::from(0i64);
    let error =
        coverage_profile(&starts, &ends, None, &keys, Some((&bound, &bound)), true).unwrap_err();
    assert!(error.to_string().contains("index 3"));
    let weights = Series::new("w".into(), [0i64, 1, -1, 0]);
    let error = coverage_profile(
        &starts,
        &starts,
        Some(&weights),
        &keys,
        Some((&bound, &bound)),
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("index 2"));
    assert!(error.to_string().contains("negative"));
    for dtype in [
        DataType::Boolean,
        DataType::Float64,
        DataType::Int128,
        DataType::Date,
    ] {
        let weights = Series::full_null("w".into(), 0, &dtype);
        assert!(matches!(
            coverage_profile(
                &starts.clear(),
                &ends.clear(),
                Some(&weights),
                &[],
                None,
                false
            ),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
    let weights = Series::new("w".into(), [Some(0i64), Some(0), None, Some(0)]);
    assert!(
        coverage_profile(
            &starts,
            &starts,
            Some(&weights),
            &[],
            Some((&bound, &bound)),
            false
        )
        .unwrap_err()
        .to_string()
        .contains("null weights")
    );
}
