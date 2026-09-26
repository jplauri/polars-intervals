use polars::prelude::*;
use polars_intervals::assign_lanes;

#[test]
fn supported_types_chunks_slices_and_core_agreement() {
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
    let raw_starts = [3i64, 0, 1, 1, 4, 1, 8];
    let raw_ends = [6i64, 5, 2, 2, 4, 1, 9];
    for dtype in dtypes {
        let convert = |raw: &[i64]| {
            let series = Series::new("endpoint".into(), raw);
            match &dtype {
                DataType::Datetime(unit, zone) => series.into_datetime(*unit, zone.clone()),
                _ => series.cast(&dtype).unwrap(),
            }
        };
        let mut starts = convert(&raw_starts[..2]);
        starts.append(&convert(&raw_starts[2..])).unwrap();
        let mut ends = convert(&raw_ends[..4]);
        ends.append(&convert(&raw_ends[4..])).unwrap();
        for (offset, len) in [(0, 7), (1, 5), (0, 0)] {
            let lanes = assign_lanes(
                &starts.slice(offset as i64, len),
                &ends.slice(offset as i64, len),
            )
            .unwrap();
            assert_eq!(lanes.name().as_str(), "assign_lanes");
            assert_eq!(lanes.dtype(), &DataType::UInt32);
            assert_eq!(lanes.null_count(), 0);
            assert_eq!(
                lanes.u32().unwrap().into_no_null_iter().collect::<Vec<_>>(),
                intervals_core::assign_lanes(
                    &raw_starts[offset..offset + len],
                    &raw_ends[offset..offset + len]
                )
                .unwrap()
            );
        }
        assert!(
            assign_lanes(&starts, &ends.slice(0, 1))
                .unwrap_err()
                .to_string()
                .contains("equal lengths")
        );
        assert!(
            assign_lanes(&convert(&[0, 5]), &convert(&[1, 4]))
                .unwrap_err()
                .to_string()
                .contains("index 1")
        );
        let nulls = Series::full_null("nulls".into(), 7, &dtype);
        for (s, e) in [(&starts, &nulls), (&nulls, &ends), (&nulls, &nulls)] {
            assert!(
                assign_lanes(s, e)
                    .unwrap_err()
                    .to_string()
                    .contains("null endpoints")
            );
        }
    }
}

#[test]
fn labels_are_not_limited_to_endpoint_width() {
    let starts = Series::new("start".into(), vec![0u8; 300]);
    let ends = Series::new("end".into(), vec![1u8; 300]);
    assert_eq!(
        assign_lanes(&starts, &ends).unwrap().n_unique().unwrap(),
        300
    );
}

#[test]
fn rejects_logical_mismatch_before_physical_conversion() {
    let integer = Series::new("s".into(), [0i64]);
    let date = integer.cast(&DataType::Date).unwrap();
    let ms = integer.clone().into_datetime(TimeUnit::Milliseconds, None);
    let us = integer.clone().into_datetime(TimeUnit::Microseconds, None);
    let utc = integer.clone().into_datetime(
        TimeUnit::Milliseconds,
        TimeZone::opt_try_new(Some("UTC")).unwrap(),
    );
    for (s, e) in [(&date, &ms), (&ms, &integer), (&ms, &us), (&ms, &utc)] {
        assert!(matches!(
            assign_lanes(s, e),
            Err(PolarsError::InvalidOperation(_))
        ));
    }
}
