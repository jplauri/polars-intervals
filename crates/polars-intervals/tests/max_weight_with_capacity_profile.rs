mod support;

use polars::prelude::*;
use polars_intervals::{max_weight_with_capacity, max_weight_with_capacity_profile as solve};
use support::{INTEGER_DTYPES, endpoint_dtypes, typed_series};

fn values(mask: Series) -> Vec<bool> {
    assert_eq!(mask.name().as_str(), "selected");
    assert_eq!(mask.dtype(), &DataType::Boolean);
    assert_eq!(mask.null_count(), 0);
    mask.bool().unwrap().no_null_iter().collect()
}

#[test]
fn integer_and_temporal_endpoints_across_independent_chunks() {
    for dtype in endpoint_dtypes() {
        let convert = |input: &[i64]| typed_series(input, &dtype);
        let mut s = convert(&[0, 0]);
        s.append(&convert(&[0, 2])).unwrap();
        let mut e = convert(&[10]);
        e.append(&convert(&[10, 10, 2])).unwrap();
        let mut ps = convert(&[5]);
        ps.append(&convert(&[0])).unwrap();
        let pe = convert(&[10, 5]);
        for weight_dtype in INTEGER_DTYPES {
            for capacity_dtype in INTEGER_DTYPES {
                let w = Series::new("w".into(), [9i64, 7, 5, 3])
                    .cast(&weight_dtype)
                    .unwrap();
                let c = Series::new("c".into(), [2i64, 3])
                    .cast(&capacity_dtype)
                    .unwrap();
                assert_eq!(
                    values(solve(&s, &e, &w, &ps, &pe, &c).unwrap()),
                    [true, true, false, true]
                );
                assert!(
                    values(
                        solve(&s.slice(0, 0), &e.slice(0, 0), &w.slice(0, 0), &ps, &pe, &c)
                            .unwrap()
                    )
                    .is_empty()
                );
                assert_eq!(
                    values(
                        solve(&s, &e, &w, &ps.slice(0, 0), &pe.slice(0, 0), &c.slice(0, 0))
                            .unwrap()
                    ),
                    [false, false, false, true]
                );
            }
        }
    }
}

#[test]
fn independent_lengths_and_validation_before_empty_fast_paths() {
    let s = Series::new("s".into(), [0i64, 0, 2]);
    let e = Series::new("e".into(), [10i64, 10, 2]);
    let w = Series::new("w".into(), [9i64, 7, 3]);
    let ps = Series::new("ps".into(), [0i64, 5]);
    let pe = Series::new("pe".into(), [5i64, 10]);
    let c = Series::new("c".into(), [2i64, 1]);
    assert_eq!(
        values(solve(&s, &e, &w, &ps, &pe, &c).unwrap()),
        [true, false, true]
    );
    let original = [&s, &e, &w, &ps, &pe, &c];
    for index in 0..6 {
        let shortened = original[index].slice(0, 1);
        let mut inputs = original;
        inputs[index] = &shortened;
        let [s, e, w, ps, pe, c] = inputs;
        assert!(
            solve(s, e, w, ps, pe, c)
                .unwrap_err()
                .to_string()
                .contains("equal lengths")
        );

        let nulls = Series::full_null("null".into(), original[index].len(), &DataType::Int64);
        inputs[index] = &nulls;
        let [s, e, w, ps, pe, c] = inputs;
        assert!(
            solve(s, e, w, ps, pe, c)
                .unwrap_err()
                .to_string()
                .contains("null")
        );
    }
    let empty = s.slice(0, 0);
    for (ps, pe, c, expected) in [
        (
            ps.clone(),
            Series::new("pe".into(), [7i64, 10]),
            c.clone(),
            "overlap",
        ),
        (
            ps.clone(),
            pe.clone(),
            Series::new("c".into(), [-1i64, 2]),
            "negative",
        ),
        (
            ps.clone(),
            Series::new("pe".into(), [0i64, 4]),
            c.clone(),
            "profile",
        ),
    ] {
        assert!(
            solve(&empty, &empty, &empty, &ps, &pe, &c)
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
    }
}

#[test]
fn exact_logical_types_and_no_implicit_integer_casts() {
    let empty = Series::new_empty("empty".into(), &DataType::Int64);
    for dtype in [
        DataType::Float64,
        DataType::Int128,
        DataType::Decimal(20, 2),
        DataType::Boolean,
    ] {
        let other = Series::new_empty("other".into(), &dtype);
        for (w, c, role) in [(&other, &empty, "weight"), (&empty, &other, "capacity")] {
            let message = solve(&empty, &empty, w, &empty, &empty, c)
                .unwrap_err()
                .to_string();
            assert!(
                message.contains(&format!("integer {role} dtype")),
                "{message}"
            );
        }
        assert!(
            solve(&other, &other, &empty, &other, &other, &empty)
                .unwrap_err()
                .to_string()
                .contains("integer dtype, Date, or Datetime")
        );
    }
    let naive = DataType::Datetime(TimeUnit::Microseconds, None);
    for (left, right) in [
        (DataType::Int32, DataType::UInt32),
        (DataType::Date, DataType::Int32),
        (DataType::Date, naive.clone()),
        (
            naive.clone(),
            DataType::Datetime(TimeUnit::Nanoseconds, None),
        ),
        (
            naive,
            DataType::Datetime(
                TimeUnit::Microseconds,
                TimeZone::opt_try_new(Some("UTC")).unwrap(),
            ),
        ),
        (
            DataType::Datetime(
                TimeUnit::Microseconds,
                TimeZone::opt_try_new(Some("UTC")).unwrap(),
            ),
            DataType::Datetime(
                TimeUnit::Microseconds,
                TimeZone::opt_try_new(Some("Europe/Helsinki")).unwrap(),
            ),
        ),
    ] {
        let jobs = Series::new_empty("jobs".into(), &left);
        let profile = Series::new_empty("profile".into(), &right);
        assert!(
            solve(&jobs, &jobs, &empty, &profile, &profile, &empty)
                .unwrap_err()
                .to_string()
                .contains("matching integer, Date, or Datetime")
        );
    }
}

#[test]
fn full_unsigned_range_and_constant_capacity_equivalence() {
    let s = Series::new("s".into(), [0u64, 0, 0, u64::MAX]);
    let e = Series::new("e".into(), [u64::MAX; 4]);
    let w = Series::new("w".into(), [u64::MAX - 2, u64::MAX, u64::MAX - 1, u64::MAX]);
    let ps = Series::new("ps".into(), [0u64]);
    let pe = Series::new("pe".into(), [u64::MAX]);
    for k in [0, 1, 2, 3, usize::MAX] {
        let c = Series::new("c".into(), [k as u64]);
        let actual = values(solve(&s, &e, &w, &ps, &pe, &c).unwrap());
        let expected = max_weight_with_capacity(&s, &e, &w, k)
            .unwrap()
            .bool()
            .unwrap()
            .no_null_iter()
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
