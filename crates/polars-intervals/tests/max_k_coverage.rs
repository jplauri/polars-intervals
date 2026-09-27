use polars::prelude::*;
use polars_intervals::max_k_coverage;

#[test]
fn endpoint_dtypes_chunks_and_row_order() {
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
        let convert = |v: &[i64]| {
            let series = Series::new("endpoint".into(), v);
            match &dtype {
                DataType::Datetime(unit, zone) => series.into_datetime(*unit, zone.clone()),
                _ => series.cast(&dtype).unwrap(),
            }
        };
        let mut s = convert(&[10]);
        s.append(&convert(&[0, 1, 8])).unwrap();
        let mut e = convert(&[18, 10]);
        e.append(&convert(&[11, 8])).unwrap();
        let result = max_k_coverage(&s, &e, 2).unwrap();
        assert_eq!(result.dtype(), &DataType::Boolean);
        assert_eq!(result.null_count(), 0);
        assert_eq!(
            result.bool().unwrap().no_null_iter().collect::<Vec<_>>(),
            [true, true, false, false]
        );
        assert!(
            max_k_coverage(&s.slice(0, 0), &e.slice(0, 0), 100)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            max_k_coverage(&s, &e, 0).unwrap().bool().unwrap().sum(),
            Some(0)
        );
    }
}

#[test]
fn validation_even_for_zero_budget() {
    let s = Series::new("s".into(), [0i64, 4]);
    let e = Series::new("e".into(), [0i64, 3]);
    for k in [0, 1, 100] {
        assert!(
            max_k_coverage(&s, &e, k)
                .unwrap_err()
                .to_string()
                .contains("index 1")
        );
        assert!(
            max_k_coverage(&s, &e.slice(0, 1), k)
                .unwrap_err()
                .to_string()
                .contains("equal lengths")
        );
        assert!(
            max_k_coverage(&s, &e.cast(&DataType::Int32).unwrap(), k)
                .unwrap_err()
                .to_string()
                .contains("matching")
        );
        assert!(
            max_k_coverage(&Series::full_null("s".into(), 2, &DataType::Int64), &e, k)
                .unwrap_err()
                .to_string()
                .contains("null endpoints")
        );
    }
}
