use polars::prelude::*;
use polars_intervals::minimum_stabbing_points;

#[test]
fn logical_list_dtype_chunks_and_empty() {
    let mut dtypes = vec![
        DataType::Int8,
        DataType::Int16,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt8,
        DataType::UInt16,
        DataType::UInt32,
        DataType::UInt64,
        DataType::Date,
    ];
    for unit in [
        TimeUnit::Milliseconds,
        TimeUnit::Microseconds,
        TimeUnit::Nanoseconds,
    ] {
        for zone in [None, Some("UTC"), Some("Europe/Helsinki")] {
            dtypes.push(DataType::Datetime(
                unit,
                TimeZone::opt_try_new(zone).unwrap(),
            ));
        }
    }
    for dtype in dtypes {
        let convert = |values: &[i64]| {
            let s = Series::new("endpoint".into(), values);
            match &dtype {
                DataType::Datetime(unit, zone) => s.into_datetime(*unit, zone.clone()),
                _ => s.cast(&dtype).unwrap(),
            }
        };
        let mut s = convert(&[5]);
        s.append(&convert(&[0, 2])).unwrap();
        let mut e = convert(&[9, 4]);
        e.append(&convert(&[6])).unwrap();
        let result = minimum_stabbing_points(&s, &e).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.null_count(), 0);
        assert_eq!(result.dtype(), &DataType::List(Box::new(dtype.clone())));
        let points = result.list().unwrap().get_as_series(0).unwrap();
        // ListChunked::get_as_series exposes the physical child in Polars 0.55.
        assert_eq!(points.dtype(), &dtype.to_physical());
        assert_eq!(
            points
                .to_physical_repr()
                .cast(&DataType::Int64)
                .unwrap()
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [3, 8]
        );
        let empty = minimum_stabbing_points(&s.slice(0, 0), &e.slice(0, 0)).unwrap();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty.dtype(), result.dtype());
        assert!(empty.list().unwrap().get_as_series(0).unwrap().is_empty());
    }
}

#[test]
fn validation_errors() {
    let s = Series::new("s".into(), [6i64, 3, 0]);
    let e = Series::new("e".into(), [9i64, 3, 2]);
    for (left, right, message) in [
        (
            s.clone(),
            e.clone(),
            "cannot stab empty interval at index 1",
        ),
        (
            e.clone(),
            s.clone(),
            "interval at index 0 has start greater than end",
        ),
        (s.slice(0, 1), e.clone(), "equal lengths"),
        (s.cast(&DataType::Int32).unwrap(), e.clone(), "matching"),
        (
            Series::full_null("s".into(), 3, &DataType::Int64),
            e.clone(),
            "null endpoints",
        ),
        (
            s.cast(&DataType::Float64).unwrap(),
            e.cast(&DataType::Float64).unwrap(),
            "integer dtype",
        ),
    ] {
        assert!(
            minimum_stabbing_points(&left, &right)
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
}
