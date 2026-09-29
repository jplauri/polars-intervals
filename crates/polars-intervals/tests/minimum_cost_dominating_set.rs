use polars::prelude::*;
use polars_intervals::minimum_cost_dominating_set as solve;

mod support;
use support::{INTEGER_DTYPES, endpoint_dtypes, typed_series};

fn values(mask: Series) -> Vec<bool> {
    assert_eq!(mask.dtype(), &DataType::Boolean);
    assert_eq!(mask.null_count(), 0);
    assert_eq!(mask.name().as_str(), "minimum_cost_dominating_set");
    mask.bool().unwrap().no_null_iter().collect()
}

#[test]
fn supported_endpoints_and_costs_preserve_chunks_and_original_order() {
    for endpoint in endpoint_dtypes() {
        let convert = |rows: &[i64]| typed_series(rows, &endpoint);
        let mut starts = convert(&[6, 2]);
        starts.append(&convert(&[0, 3, 2, 15])).unwrap();
        let mut ends = convert(&[10]);
        ends.append(&convert(&[2, 4, 7, 2, 16])).unwrap();
        assert_eq!(
            values(solve(&starts, &ends, None).unwrap()),
            [false, true, false, true, true, true]
        );
        for dtype in INTEGER_DTYPES {
            let mut costs = Series::new("c".into(), [1i64, 0, 1]).cast(&dtype).unwrap();
            costs
                .append(&Series::new("c".into(), [10i64, 0, 0]).cast(&dtype).unwrap())
                .unwrap();
            assert_eq!(
                values(solve(&starts, &ends, Some(&costs)).unwrap()),
                [true, true, true, false, true, true]
            );
            let ones = Series::new("ones".into(), [1i64; 6]).cast(&dtype).unwrap();
            assert_eq!(
                values(solve(&starts, &ends, Some(&ones)).unwrap()),
                [false, true, false, true, true, true]
            );
            let zeros = Series::new("zeros".into(), [0i64; 6]).cast(&dtype).unwrap();
            assert_eq!(
                values(solve(&starts, &ends, Some(&zeros)).unwrap()),
                [false, true, false, true, true, true]
            );
            for explicit in [None, Some(costs.slice(0, 0))] {
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
fn exact_integer_and_temporal_boundaries_and_large_costs() {
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
        let starts = typed_series(&[minimum, maximum - 2, maximum - 1, maximum], &endpoint);
        let ends = typed_series(&[maximum - 2, maximum - 1, maximum, maximum], &endpoint);
        let costs = Series::new("c".into(), [u64::MAX; 4]);
        assert_eq!(
            values(solve(&starts, &ends, Some(&costs)).unwrap()),
            [true; 4]
        );
    }
    let starts = Series::new("s".into(), [u64::MAX - 2, u64::MAX - 2, u64::MAX]);
    let ends = Series::new("e".into(), [u64::MAX, u64::MAX, u64::MAX]);
    let costs = Series::new("c".into(), [u64::MAX, u64::MAX - 1, u64::MAX]);
    assert_eq!(
        values(solve(&starts, &ends, Some(&costs)).unwrap()),
        [false, true, true]
    );
}

#[test]
fn zeros_and_repeated_empty_vertices_are_mandatory() {
    let starts = Series::new("s".into(), [0i64, 1, 1, 5]);
    let ends = Series::new("e".into(), [5i64, 1, 1, 5]);
    for costs in [None, Some(Series::new("c".into(), [0i64, 0, 1, 0]))] {
        assert_eq!(
            values(solve(&starts, &ends, costs.as_ref()).unwrap()),
            [true; 4]
        );
    }
    let point = Series::new("s".into(), [1i64]);
    let end = Series::new("e".into(), [3i64]);
    let zero = Series::new("c".into(), [0i64]);
    assert_eq!(values(solve(&point, &end, Some(&zero)).unwrap()), [true]);
}

#[test]
fn every_row_is_validated_before_pruning_with_original_error_indices() {
    for (starts, ends) in [([0i64, 5, 2], [0i64, 5, 1]), ([0, 0, 5], [10, 10, 4])] {
        let starts = Series::new("s".into(), starts);
        let ends = Series::new("e".into(), ends);
        for costs in [None, Some(Series::new("c".into(), [0i64; 3]))] {
            let error = solve(&starts, &ends, costs.as_ref())
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
    for (starts, ends) in [([0i64, 1, 5], [0i64, 1, 5]), ([0, 1, 0], [10, 2, 10])] {
        let error = solve(
            &Series::new("s".into(), starts),
            &Series::new("e".into(), ends),
            Some(&Series::new("c".into(), [0i64, 1, i64::MIN])),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("index 2"), "{error}");
        assert!(error.contains("negative"), "{error}");
    }
}

#[test]
fn invalid_cost_lengths_nulls_and_dtypes_are_errors() {
    let starts = Series::new("s".into(), [0i64, 1, 2]);
    let ends = Series::new("e".into(), [2i64, 3, 4]);
    for (costs, message) in [
        (Series::new("c".into(), [1i64]), "equal lengths"),
        (
            Series::full_null("c".into(), 3, &DataType::Int64),
            "null costs",
        ),
    ] {
        let error = solve(&starts, &ends, Some(&costs)).unwrap_err().to_string();
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
            assert!(error.contains("integer cost dtype"), "{error}");
        }
    }
}
