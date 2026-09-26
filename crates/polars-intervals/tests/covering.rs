use polars::prelude::*;
use polars_intervals::{minimum_cost_cover, minimum_cover};

fn scalar(series: &Series, index: usize) -> Scalar {
    Scalar::new(
        series.dtype().clone(),
        series.get(index).unwrap().into_static(),
    )
}

#[test]
fn endpoints_costs_chunks_and_scalars() {
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
            let s = Series::new("endpoint".into(), values);
            match &dtype {
                DataType::Datetime(unit, zone) => s.into_datetime(*unit, zone.clone()),
                _ => s.cast(&dtype).unwrap(),
            }
        };
        let mut s = convert(&[0]);
        s.append(&convert(&[0, 5, 3])).unwrap();
        let mut e = convert(&[10, 5]);
        e.append(&convert(&[10, 3])).unwrap();
        let (left, right) = (scalar(&s, 0), scalar(&e, 0));
        assert_eq!(
            minimum_cover(&s, &e, &left, &right)
                .unwrap()
                .bool()
                .unwrap()
                .no_null_iter()
                .collect::<Vec<_>>(),
            [true, false, false, false]
        );
        for cost_type in &integers {
            let w = Series::new("cost".into(), [100i64, 10, 10, 0])
                .cast(cost_type)
                .unwrap();
            let result = minimum_cost_cover(&s, &e, &w, &left, &right).unwrap();
            assert_eq!(result.dtype(), &DataType::Boolean);
            assert_eq!(
                result.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
                [false, true, true, false]
            );
            assert_eq!(
                minimum_cost_cover(&s, &e, &w, &left, &left)
                    .unwrap()
                    .bool()
                    .unwrap()
                    .sum(),
                Some(0)
            );
            assert!(
                minimum_cost_cover(
                    &s.slice(0, 0),
                    &e.slice(0, 0),
                    &w.slice(0, 0),
                    &left,
                    &right
                )
                .unwrap_err()
                .to_string()
                .contains("cannot be covered")
            );
        }
        assert!(minimum_cover(&s, &e, &Scalar::null(dtype.clone()), &right).is_err());
        assert!(
            minimum_cover(&s, &e, &right, &left)
                .unwrap_err()
                .to_string()
                .contains("target start")
        );
    }
}

#[test]
fn target_metadata_and_validation() {
    let s = Series::new("s".into(), [0i64, 5]);
    let e = Series::new("e".into(), [5i64, 10]);
    let left = Scalar::from(0i64);
    let right = Scalar::from(10i64);
    assert!(
        minimum_cover(&s, &e, &Scalar::from(0i32), &right)
            .unwrap_err()
            .to_string()
            .contains("exactly match")
    );
    for dtype in [
        DataType::Datetime(TimeUnit::Microseconds, None),
        DataType::Datetime(TimeUnit::Nanoseconds, Some(TimeZone::UTC)),
        DataType::Date,
    ] {
        let starts = s.cast(&dtype).unwrap();
        let ends = e.cast(&dtype).unwrap();
        assert!(minimum_cover(&starts, &ends, &left, &right).is_err());
    }
    for (costs, message) in [
        (Series::new("c".into(), [1i64, -1]), "negative"),
        (Series::new("c".into(), [1f64, 2.0]), "integer cost dtype"),
        (
            Series::full_null("c".into(), 2, &DataType::Int64),
            "null costs",
        ),
        (
            Series::new("c".into(), [1i64]),
            "equal interval and cost lengths",
        ),
    ] {
        assert!(
            minimum_cost_cover(&s, &e, &costs, &left, &right)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
    assert!(
        minimum_cover(&e, &s, &left, &right)
            .unwrap_err()
            .to_string()
            .contains("index 0")
    );
    assert!(
        minimum_cover(&s.slice(0, 1), &e, &left, &right)
            .unwrap_err()
            .to_string()
            .contains("equal lengths")
    );
}
