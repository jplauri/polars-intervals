use polars::prelude::*;

pub const INTEGER_DTYPES: [DataType; 8] = [
    DataType::Int8,
    DataType::Int16,
    DataType::Int32,
    DataType::Int64,
    DataType::UInt8,
    DataType::UInt16,
    DataType::UInt32,
    DataType::UInt64,
];

pub fn endpoint_dtypes() -> Vec<DataType> {
    let mut dtypes = INTEGER_DTYPES.to_vec();
    dtypes.push(DataType::Date);
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
    dtypes
}

pub fn typed_series(values: &[i64], dtype: &DataType) -> Series {
    let series = Series::new("endpoint".into(), values);
    match dtype {
        // Preserve timezone metadata without requiring Polars' timezone feature.
        DataType::Datetime(unit, zone) => series.into_datetime(*unit, zone.clone()),
        _ => series.cast(dtype).unwrap(),
    }
}
