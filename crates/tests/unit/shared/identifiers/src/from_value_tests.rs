//! Coverage for the `FromDbValue` conversions in `db_value/from_value.rs`,
//! including the coercion branches, error paths, and `ToDbValue` round-trips.

use chrono::{DateTime, TimeZone, Utc};
use systemprompt_identifiers::{DbValue, FromDbValue, ToDbValue};

#[test]
fn string_from_each_scalar() {
    assert_eq!(
        String::from_db_value(&DbValue::String("x".to_owned())).unwrap(),
        "x"
    );
    assert_eq!(String::from_db_value(&DbValue::Int(42)).unwrap(), "42");
    assert_eq!(String::from_db_value(&DbValue::Bool(true)).unwrap(), "true");
    assert_eq!(String::from_db_value(&DbValue::Float(1.5)).unwrap(), "1.5");
    let arr = DbValue::StringArray(vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(String::from_db_value(&arr).unwrap(), "[\"a\",\"b\"]");
}

#[test]
fn string_from_timestamp_is_rfc3339() {
    let dt = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    let out = String::from_db_value(&DbValue::Timestamp(dt)).unwrap();
    assert_eq!(out, dt.to_rfc3339());
}

#[test]
fn string_rejects_null_and_bytes() {
    assert!(String::from_db_value(&DbValue::NullString).is_err());
    assert!(String::from_db_value(&DbValue::Bytes(vec![1])).is_err());
}

#[test]
fn i64_from_variants() {
    assert_eq!(i64::from_db_value(&DbValue::Int(5)).unwrap(), 5);
    assert_eq!(i64::from_db_value(&DbValue::Bool(true)).unwrap(), 1);
    assert_eq!(i64::from_db_value(&DbValue::Bool(false)).unwrap(), 0);
    assert_eq!(i64::from_db_value(&DbValue::Float(3.0)).unwrap(), 3);
    assert_eq!(
        i64::from_db_value(&DbValue::String("17".to_owned())).unwrap(),
        17
    );
}

#[test]
fn i64_error_paths() {
    assert!(i64::from_db_value(&DbValue::String("nan".to_owned())).is_err());
    assert!(i64::from_db_value(&DbValue::StringArray(vec![])).is_err());
    assert!(i64::from_db_value(&DbValue::Timestamp(Utc::now())).is_err());
    assert!(i64::from_db_value(&DbValue::NullInt).is_err());
    assert!(i64::from_db_value(&DbValue::Bytes(vec![1])).is_err());
    assert!(i64::from_db_value(&DbValue::Float(f64::NAN)).is_err());
    assert!(i64::from_db_value(&DbValue::Float(f64::INFINITY)).is_err());
}

#[test]
fn i32_and_unsigned_conversions_and_range() {
    assert_eq!(i32::from_db_value(&DbValue::Int(10)).unwrap(), 10);
    assert!(i32::from_db_value(&DbValue::Int(i64::MAX)).is_err());
    assert_eq!(u64::from_db_value(&DbValue::Int(8)).unwrap(), 8);
    assert!(u64::from_db_value(&DbValue::Int(-1)).is_err());
    assert_eq!(u32::from_db_value(&DbValue::Int(9)).unwrap(), 9);
    assert!(u32::from_db_value(&DbValue::Int(-5)).is_err());
}

#[test]
fn f64_from_variants_and_errors() {
    assert!((f64::from_db_value(&DbValue::Float(2.5)).unwrap() - 2.5).abs() < f64::EPSILON);
    assert!((f64::from_db_value(&DbValue::Int(4)).unwrap() - 4.0).abs() < f64::EPSILON);
    assert!(
        (f64::from_db_value(&DbValue::String("1.25".to_owned())).unwrap() - 1.25).abs()
            < f64::EPSILON
    );
    assert!(f64::from_db_value(&DbValue::String("xx".to_owned())).is_err());
    assert!(f64::from_db_value(&DbValue::StringArray(vec![])).is_err());
    assert!(f64::from_db_value(&DbValue::Timestamp(Utc::now())).is_err());
    assert!(f64::from_db_value(&DbValue::NullFloat).is_err());
    assert!(f64::from_db_value(&DbValue::Bool(true)).is_err());
    assert!(f64::from_db_value(&DbValue::Bytes(vec![1])).is_err());
    assert!(f64::from_db_value(&DbValue::Int(1 << 60)).is_err());
}

#[test]
fn bool_from_variants_and_errors() {
    assert!(bool::from_db_value(&DbValue::Bool(true)).unwrap());
    assert!(bool::from_db_value(&DbValue::Int(1)).unwrap());
    assert!(!bool::from_db_value(&DbValue::Int(0)).unwrap());
    assert!(bool::from_db_value(&DbValue::String("YES".to_owned())).unwrap());
    assert!(!bool::from_db_value(&DbValue::String("no".to_owned())).unwrap());
    assert!(bool::from_db_value(&DbValue::String("1".to_owned())).unwrap());
    assert!(bool::from_db_value(&DbValue::String("maybe".to_owned())).is_err());
    assert!(bool::from_db_value(&DbValue::StringArray(vec![])).is_err());
    assert!(bool::from_db_value(&DbValue::Timestamp(Utc::now())).is_err());
    assert!(bool::from_db_value(&DbValue::NullBool).is_err());
    assert!(bool::from_db_value(&DbValue::Float(1.0)).is_err());
    assert!(bool::from_db_value(&DbValue::Bytes(vec![1])).is_err());
}

#[test]
fn bytes_from_variants_and_errors() {
    assert_eq!(
        Vec::<u8>::from_db_value(&DbValue::Bytes(vec![7, 8])).unwrap(),
        vec![7, 8]
    );
    assert_eq!(
        Vec::<u8>::from_db_value(&DbValue::String("ab".to_owned())).unwrap(),
        b"ab".to_vec()
    );
    assert!(Vec::<u8>::from_db_value(&DbValue::NullBytes).is_err());
    assert!(Vec::<u8>::from_db_value(&DbValue::Int(1)).is_err());
}

#[test]
fn option_from_null_is_none_and_value_is_some() {
    assert_eq!(
        Option::<i64>::from_db_value(&DbValue::NullInt).unwrap(),
        None
    );
    assert_eq!(
        Option::<i64>::from_db_value(&DbValue::Int(3)).unwrap(),
        Some(3)
    );
    assert_eq!(
        Option::<String>::from_db_value(&DbValue::String("v".to_owned())).unwrap(),
        Some("v".to_owned())
    );
}

#[test]
fn datetime_from_string_timestamp_and_int() {
    let dt = Utc.with_ymd_and_hms(2026, 6, 22, 0, 0, 0).unwrap();
    let parsed = DateTime::<Utc>::from_db_value(&DbValue::String(dt.to_rfc3339())).unwrap();
    assert_eq!(parsed, dt);
    assert_eq!(
        DateTime::<Utc>::from_db_value(&DbValue::Timestamp(dt)).unwrap(),
        dt
    );
    let from_epoch = DateTime::<Utc>::from_db_value(&DbValue::Int(0)).unwrap();
    assert_eq!(from_epoch.timestamp(), 0);
}

#[test]
fn datetime_error_paths() {
    assert!(DateTime::<Utc>::from_db_value(&DbValue::String("not-a-date".to_owned())).is_err());
    assert!(DateTime::<Utc>::from_db_value(&DbValue::NullTimestamp).is_err());
    assert!(DateTime::<Utc>::from_db_value(&DbValue::Bool(true)).is_err());
}

#[test]
fn round_trip_scalars_through_db_value() {
    let original: i64 = -123;
    let back = i64::from_db_value(&original.to_db_value()).unwrap();
    assert_eq!(back, original);

    let s = String::from("round");
    assert_eq!(String::from_db_value(&s.to_db_value()).unwrap(), s);

    let b = true;
    assert_eq!(bool::from_db_value(&b.to_db_value()).unwrap(), b);

    let dt = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();
    assert_eq!(
        DateTime::<Utc>::from_db_value(&dt.to_db_value()).unwrap(),
        dt
    );
}

#[test]
fn f64_beyond_i64_range_is_out_of_range_not_wrapped() {
    assert!(i64::from_db_value(&DbValue::Float(f64::MAX)).is_err());
    assert!(i64::from_db_value(&DbValue::Float(f64::MIN)).is_err());
}

#[test]
fn gateway_conversation_id_rejects_wrong_length_prefix_and_non_hex() {
    use systemprompt_identifiers::GatewayConversationId;

    let short = GatewayConversationId::try_new("bad-prefix").unwrap_err();
    assert!(short.to_string().contains("16 hex"), "got: {short}");

    let wrong_prefix = GatewayConversationId::try_new("xtx_0123456789abcdef").unwrap_err();
    assert!(
        wrong_prefix.to_string().contains("missing 'ctx_' prefix"),
        "got: {wrong_prefix}"
    );

    let upper_hex = GatewayConversationId::try_new("ctx_0123456789ABCDEF").unwrap_err();
    assert!(
        upper_hex.to_string().contains("lowercase hex"),
        "got: {upper_hex}"
    );
}
