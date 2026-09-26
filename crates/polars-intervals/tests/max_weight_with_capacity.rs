use polars::prelude::*;
use polars_intervals::{max_weight_non_overlapping, max_weight_with_capacity as solve};

#[test]
fn chunks_integer_temporal_and_weight_widths() {
    let integers = [
        DataType::Int8,
        DataType::Int16,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt8,
        DataType::UInt16,
        DataType::UInt32,
        DataType::UInt64,
    ];
    let mut endpoints = integers.to_vec();
    endpoints.push(DataType::Date);
    for unit in [
        TimeUnit::Milliseconds,
        TimeUnit::Microseconds,
        TimeUnit::Nanoseconds,
    ] {
        for zone in [None, Some("UTC"), Some("Europe/Helsinki")] {
            endpoints.push(DataType::Datetime(
                unit,
                TimeZone::opt_try_new(zone).unwrap(),
            ));
        }
    }
    for dtype in endpoints {
        let convert = |values: &[i64]| {
            let series = Series::new("endpoint".into(), values);
            match &dtype {
                DataType::Datetime(unit, zone) => series.into_datetime(*unit, zone.clone()),
                _ => series.cast(&dtype).unwrap(),
            }
        };
        let mut s = convert(&[0, 0]);
        s.append(&convert(&[0, 2])).unwrap();
        let mut e = convert(&[5]);
        e.append(&convert(&[5, 5, 2])).unwrap();
        for weight_dtype in &integers {
            let w = Series::new("w".into(), [9i64, 7, 5, 3])
                .cast(weight_dtype)
                .unwrap();
            for (k, expected) in [
                (0, vec![false, false, false, true]),
                (1, vec![true, false, false, true]),
                (2, vec![true, true, false, true]),
                (64, vec![true; 4]),
            ] {
                let mask = solve(&s, &e, &w, k).unwrap();
                assert_eq!(mask.name().as_str(), "max_weight_with_capacity");
                assert_eq!(mask.dtype(), &DataType::Boolean);
                assert_eq!(mask.null_count(), 0);
                assert_eq!(
                    mask.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(
                    solve(&s.slice(0, 0), &e.slice(0, 0), &w.slice(0, 0), k)
                        .unwrap()
                        .len(),
                    0
                );
                if k == 1 {
                    assert_eq!(
                        mask.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
                        max_weight_non_overlapping(&s, &e, &w)
                            .unwrap()
                            .bool()
                            .unwrap()
                            .no_null_iter()
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn native_validation_and_precision() {
    let s = Series::new("s".into(), [0i64; 3]);
    let e = Series::new("e".into(), [2i64; 3]);
    let w = Series::new("w".into(), [u64::MAX - 2, u64::MAX, u64::MAX - 1]);
    assert_eq!(
        solve(&s, &e, &w, 2)
            .unwrap()
            .bool()
            .unwrap()
            .no_null_iter()
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    for k in [0, 1, 2] {
        for (s, e, w, message) in [
            (s.clone(), e.clone(), w.slice(0, 1), "equal lengths"),
            (s.clone(), e.slice(0, 1), w.clone(), "equal lengths"),
            (e.clone(), s.clone(), w.clone(), "index 0"),
            (
                s.clone(),
                e.cast(&DataType::Int32).unwrap(),
                w.clone(),
                "matching integer",
            ),
            (
                Series::full_null("s".into(), 3, &DataType::Int64),
                e.clone(),
                w.clone(),
                "null endpoints",
            ),
            (
                s.clone(),
                e.clone(),
                Series::full_null("w".into(), 3, &DataType::UInt64),
                "null weights",
            ),
        ] {
            assert!(
                solve(&s, &e, &w, k)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
        for dtype in [
            DataType::Float64,
            DataType::Int128,
            DataType::Decimal(20, 2),
        ] {
            let weights = Series::full_null("w".into(), 3, &dtype);
            let error = solve(&s, &e, &weights, k).unwrap_err().to_string();
            assert!(
                error.contains("max_weight_with_capacity")
                    && error.contains("integer weight dtype")
            );
        }
    }
}
