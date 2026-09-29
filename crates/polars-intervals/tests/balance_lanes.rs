use polars::prelude::*;
use polars_intervals::{assign_balanced_lanes, assign_lanes};

mod support;
use support::{INTEGER_DTYPES, endpoint_dtypes, typed_series};

fn labels(series: &Series) -> Vec<u32> {
    assert_eq!(series.dtype(), &DataType::UInt32);
    assert_eq!(series.null_count(), 0);
    series.u32().unwrap().into_no_null_iter().collect()
}

#[test]
fn supported_types_chunks_slices_and_core_agreement() {
    let raw_starts = [3i64, 0, 1, 1, 4, 1, 8];
    let raw_ends = [6i64, 5, 2, 2, 4, 1, 9];
    for dtype in endpoint_dtypes() {
        let convert = |values: &[i64]| typed_series(values, &dtype);
        let mut starts = convert(&raw_starts[..2]);
        starts.append(&convert(&raw_starts[2..])).unwrap();
        let mut ends = convert(&raw_ends[..4]);
        ends.append(&convert(&raw_ends[4..])).unwrap();
        for (offset, len) in [(0, 7), (1, 5), (0, 0)] {
            let starts = starts.slice(offset as i64, len);
            let ends = ends.slice(offset as i64, len);
            let original = assign_lanes(&starts, &ends).unwrap();
            let raw_s = &raw_starts[offset..offset + len];
            let raw_e = &raw_ends[offset..offset + len];
            for work in [0, 1, 100_000, u64::MAX] {
                let constructed = assign_balanced_lanes(&starts, &ends, None, work).unwrap();
                assert_eq!(constructed.name().as_str(), "assign_balanced_lanes");
                assert_eq!(
                    labels(&constructed),
                    intervals_core::assign_balanced_lanes(raw_s, raw_e, None, work).unwrap()
                );
                for lane_dtype in INTEGER_DTYPES {
                    let lanes = original.cast(&lane_dtype).unwrap();
                    let mut chunked = lanes.slice(0, 1);
                    chunked.append(&lanes.slice(1, lanes.len())).unwrap();
                    let repaired =
                        assign_balanced_lanes(&starts, &ends, Some(&chunked), work).unwrap();
                    assert_eq!(repaired.name().as_str(), "assign_balanced_lanes");
                    assert_eq!(
                        labels(&repaired),
                        intervals_core::assign_balanced_lanes(
                            raw_s,
                            raw_e,
                            Some(&labels(&original)),
                            work
                        )
                        .unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn empty_rows_count_toward_balance_but_never_add_lanes() {
    let starts = Series::new("s".into(), [0i64, 0, 1, 1]);
    let ends = Series::new("e".into(), [3i64, 3, 1, 1]);
    let original = assign_lanes(&starts, &ends).unwrap();
    assert_eq!(&labels(&original)[2..], &[0, 0]);
    for result in [
        assign_balanced_lanes(&starts, &ends, None, 100_000).unwrap(),
        assign_balanced_lanes(&starts, &ends, Some(&original), 100_000).unwrap(),
    ] {
        let values = labels(&result);
        assert_eq!(values.iter().filter(|&&id| id == 0).count(), 2);
        assert_eq!(values.iter().filter(|&&id| id == 1).count(), 2);
        assert_ne!(values[0], values[1]);
    }
    let empty = Series::new("s".into(), [1u64; 4]);
    assert_eq!(
        labels(&assign_balanced_lanes(&empty, &empty, None, 100_000).unwrap()),
        [0; 4]
    );
}

#[test]
fn zero_budget_preserves_arbitrary_valid_labels() {
    let starts = Series::new("s".into(), [0i64; 3]);
    let ends = Series::new("e".into(), [3i64; 3]);
    let original = Series::new("lane".into(), [2u64, 0, 1]);
    assert_eq!(
        labels(&assign_balanced_lanes(&starts, &ends, Some(&original), 0).unwrap()),
        [2, 0, 1]
    );
    assert_eq!(
        labels(&assign_balanced_lanes(&starts, &ends, None, 0).unwrap()),
        labels(&assign_lanes(&starts, &ends).unwrap())
    );
}

#[test]
fn lane_lengths_must_match_even_with_zero_budget() {
    let starts = Series::new("s".into(), [0i64, 5, 6]);
    let lanes = Series::new("lane".into(), [0u32]);
    for work in [0, 100_000] {
        let result = assign_balanced_lanes(&starts, &starts, Some(&lanes), work);
        assert!(result.unwrap_err().to_string().contains("equal lengths"));
    }
}

#[test]
fn lane_values_are_checked_without_lossy_casts_or_unchecked_palette_allocations() {
    let starts = Series::new("s".into(), [0i64, 0]);
    let ends = Series::new("e".into(), [2i64, 2]);
    for lanes in [
        Series::new("lane".into(), [Some(0i64), None]),
        Series::new("lane".into(), [0i64, -1]),
        Series::new("lane".into(), [0u64, u64::MAX]),
        Series::new("lane".into(), [0u64, u64::from(u32::MAX) + 1]),
        Series::new("lane".into(), [0u32, u32::MAX]),
        Series::new("lane".into(), [0u32, 2]),
        Series::new("lane".into(), [1u32, 2]),
        Series::new("lane".into(), [0u32, 0]),
    ] {
        for work in [0, 100_000] {
            assert!(assign_balanced_lanes(&starts, &ends, Some(&lanes), work).is_err());
        }
    }
    for dtype in [
        DataType::Boolean,
        DataType::Float32,
        DataType::Float64,
        DataType::String,
        DataType::Int128,
        DataType::Date,
        DataType::Datetime(TimeUnit::Microseconds, None),
        DataType::Decimal(20, 0),
    ] {
        for n in [0, 2] {
            let lanes = Series::full_null("lane".into(), n, &dtype);
            assert!(
                assign_balanced_lanes(&starts.slice(0, n), &ends.slice(0, n), Some(&lanes), 0)
                    .is_err()
            );
        }
    }
}

#[test]
fn rejects_nonminimum_even_when_proper_and_equitable() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let ends = Series::new("e".into(), [1i64, 2]);
    let lanes = Series::new("lane".into(), [0u32, 1]);
    for work in [0, 100_000] {
        for e in [&starts, &ends] {
            assert!(
                assign_balanced_lanes(&starts, e, Some(&lanes), work)
                    .unwrap_err()
                    .to_string()
                    .contains("minimum")
            );
        }
    }
}
