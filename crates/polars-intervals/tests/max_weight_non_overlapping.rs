use polars::prelude::*;
use polars_intervals::max_weight_non_overlapping as solve;

#[test]
fn integer_and_temporal_endpoints_with_all_weight_widths() {
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
    for endpoint in endpoints {
        let convert = |raw: &[i64]| {
            let series = Series::new("endpoint".into(), raw);
            match &endpoint {
                DataType::Datetime(unit, zone) => series.into_datetime(*unit, zone.clone()),
                _ => series.cast(&endpoint).unwrap(),
            }
        };
        let mut s = convert(&[0, 0]);
        s.append(&convert(&[4, 7, 2])).unwrap();
        let mut e = convert(&[10]);
        e.append(&convert(&[4, 7, 10, 2])).unwrap();
        for dtype in &integers {
            let mut w = Series::new("w".into(), [15i64, 10, 10])
                .cast(dtype)
                .unwrap();
            w.append(&Series::new("w".into(), [10i64, 3]).cast(dtype).unwrap())
                .unwrap();
            let mask = solve(&s, &e, &w).unwrap();
            assert_eq!(mask.dtype(), &DataType::Boolean);
            assert_eq!(mask.null_count(), 0);
            assert_eq!(mask.name().as_str(), "max_weight_non_overlapping");
            assert_eq!(
                mask.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
                [false, true, true, true, true]
            );
            assert_eq!(
                solve(&s.slice(0, 0), &e.slice(0, 0), &w.slice(0, 0))
                    .unwrap()
                    .len(),
                0
            );
            assert_eq!(
                solve(&s.slice(2, 3), &e.slice(2, 3), &w.slice(2, 3))
                    .unwrap()
                    .bool()
                    .unwrap()
                    .sum(),
                Some(3)
            );
        }
    }
}

#[test]
fn validation_and_integer_precision() {
    let s = Series::new("s".into(), [0i64, 0, 2]);
    let e = Series::new("e".into(), [2i64, 2, 3]);
    let w = Series::new("w".into(), [u64::MAX - 1, u64::MAX, u64::MAX]);
    for dtype in [DataType::Int128, DataType::Decimal(20, 2)] {
        let unsupported = Series::full_null("w".into(), 3, &dtype);
        assert!(
            solve(&s, &e, &unsupported)
                .unwrap_err()
                .to_string()
                .contains("integer weight dtype")
        );
    }
    assert_eq!(
        solve(&s, &e, &w)
            .unwrap()
            .bool()
            .unwrap()
            .no_null_iter()
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    for (s, e, w, message) in [
        (s.clone(), e.clone(), w.slice(0, 1), "equal lengths"),
        (s.slice(0, 1), e.clone(), w.slice(0, 1), "equal lengths"),
        (e.clone(), s.clone(), w.clone(), "index 0"),
        (
            s.clone(),
            e.cast(&DataType::Int32).unwrap(),
            w.clone(),
            "matching integer",
        ),
        (
            s.clone(),
            e.clone(),
            w.cast(&DataType::Float64).unwrap(),
            "integer weight dtype",
        ),
        (
            s.clone(),
            e.clone(),
            Series::full_null("w".into(), 3, &DataType::Int64),
            "null weights",
        ),
        (
            Series::full_null("s".into(), 3, &DataType::Int64),
            e.clone(),
            w.clone(),
            "null endpoints",
        ),
    ] {
        assert!(solve(&s, &e, &w).unwrap_err().to_string().contains(message));
    }
}
