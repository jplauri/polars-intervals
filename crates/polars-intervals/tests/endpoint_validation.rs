//! Endpoint validation shared by every two-column adapter function.
use polars::prelude::*;
use polars_intervals::*;

mod support;
use support::{endpoint_dtypes, typed_series};

type Adapter = fn(&Series, &Series) -> PolarsResult<Series>;

// Non-null integer columns of the starts' length, for weight, cost, and lane arguments.
fn ones(starts: &Series) -> Series {
    Series::new("w".into(), vec![1i64; starts.len()])
}

fn zeros(starts: &Series) -> Series {
    Series::new("lane".into(), vec![0u32; starts.len()])
}

// Fast-path settings (k=0, capacity=0, max_work=0) must validate like the rest.
const ADAPTERS: [(&str, Adapter); 17] = [
    ("coverage_profile_units", |s, e| {
        coverage_profile(s, e, None, &[], None, false)
            .map(|frame| frame["load"].as_materialized_series().clone())
    }),
    ("coverage_profile_weights", |s, e| {
        coverage_profile(s, e, Some(&ones(s)), &[], None, true)
            .map(|frame| frame["load"].as_materialized_series().clone())
    }),
    ("overlap_count", overlap_count),
    ("containment_count", containment_count),
    ("nesting_depth", nesting_depth),
    ("assign_lanes", assign_lanes),
    ("balanced_construct", |s, e| {
        assign_balanced_lanes(s, e, None, 0)
    }),
    ("balanced_repair", |s, e| {
        assign_balanced_lanes(s, e, Some(&zeros(s)), 0)
    }),
    ("max_k_coverage_k0", |s, e| max_k_coverage(s, e, 0)),
    ("max_k_coverage_k2", |s, e| max_k_coverage(s, e, 2)),
    ("minimum_stabbing_points", minimum_stabbing_points),
    ("domination_units", |s, e| {
        minimum_cost_dominating_set(s, e, None)
    }),
    ("domination_costs", |s, e| {
        minimum_cost_dominating_set(s, e, Some(&ones(s)))
    }),
    ("max_weight_clique", |s, e| max_weight_clique(s, e, None)),
    ("max_weight_non_overlapping", |s, e| {
        max_weight_non_overlapping(s, e, &ones(s))
    }),
    ("capacity_0", |s, e| {
        max_weight_with_capacity(s, e, &ones(s), 0)
    }),
    ("capacity_2", |s, e| {
        max_weight_with_capacity(s, e, &ones(s), 2)
    }),
];

#[test]
fn reversed_and_null_endpoints() {
    for dtype in endpoint_dtypes() {
        let starts = typed_series(&[0, 3, 4], &dtype);
        let ends = typed_series(&[1, 2, 1], &dtype);
        let nulls = Series::full_null("nulls".into(), 3, &dtype);
        for (name, adapter) in ADAPTERS {
            let error = adapter(&starts, &ends).unwrap_err();
            assert!(matches!(error, PolarsError::ComputeError(_)), "{name}");
            assert!(error.to_string().contains("index 1"), "{name}: {error}");
            for (s, e) in [(&nulls, &ends), (&starts, &nulls), (&nulls, &nulls)] {
                let error = adapter(s, e).unwrap_err();
                assert!(matches!(error, PolarsError::ComputeError(_)), "{name}");
                assert!(error.to_string().contains("null endpoints"), "{name}");
            }
        }
    }
}

#[test]
fn logical_dtypes_must_match_exactly() {
    let utc = TimeZone::opt_try_new(Some("UTC")).unwrap();
    let helsinki = TimeZone::opt_try_new(Some("Europe/Helsinki")).unwrap();
    let ms = DataType::Datetime(TimeUnit::Milliseconds, None);
    let us = DataType::Datetime(TimeUnit::Microseconds, None);
    for (a, b) in [
        (DataType::Int32, DataType::Int64),
        (DataType::Int64, DataType::UInt64),
        (DataType::Date, DataType::Int32),
        (DataType::Date, ms.clone()),
        (ms.clone(), DataType::Int64),
        (ms, us.clone()),
        (us, DataType::Datetime(TimeUnit::Microseconds, utc.clone())),
        (
            DataType::Datetime(TimeUnit::Nanoseconds, utc),
            DataType::Datetime(TimeUnit::Nanoseconds, helsinki),
        ),
    ] {
        for data in [&[][..], &[0i64][..]] {
            let (s, e) = (typed_series(data, &a), typed_series(data, &b));
            for (name, adapter) in ADAPTERS {
                for (x, y) in [(&s, &e), (&e, &s)] {
                    let error = adapter(x, y).unwrap_err();
                    assert!(matches!(error, PolarsError::InvalidOperation(_)), "{name}");
                    let message = error.to_string();
                    assert!(
                        message.contains("matching integer, Date, or Datetime"),
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn unsupported_dtypes_even_when_empty() {
    for dtype in [
        DataType::Float32,
        DataType::Float64,
        DataType::Boolean,
        DataType::String,
        DataType::Null,
        DataType::Time,
        DataType::Duration(TimeUnit::Microseconds),
        DataType::Int128,
        DataType::Decimal(20, 0),
    ] {
        for length in [0, 2] {
            let values = Series::full_null("x".into(), length, &dtype);
            for (name, adapter) in ADAPTERS {
                let error = adapter(&values, &values).unwrap_err();
                assert!(matches!(error, PolarsError::InvalidOperation(_)), "{name}");
                let message = error.to_string();
                assert!(
                    message.contains("integer dtype, Date, or Datetime"),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn lengths_must_match_without_broadcasting() {
    let starts = Series::new("s".into(), [0i64, 1]);
    let ends = Series::new("e".into(), [3i64, 4]);
    for (name, adapter) in ADAPTERS {
        for (s, e) in [(&starts, &ends.slice(0, 1)), (&starts.slice(0, 0), &ends)] {
            let error = adapter(s, e).unwrap_err();
            assert!(matches!(error, PolarsError::ShapeMismatch(_)), "{name}");
            assert!(error.to_string().contains("equal lengths"), "{name}");
        }
    }
}
