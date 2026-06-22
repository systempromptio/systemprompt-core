//! Coverage for the `ToDbValue` conversions over primitive and reference
//! types in `db_value/to_value.rs`.

use chrono::{DateTime, TimeZone, Utc};
use systemprompt_identifiers::{DbValue, ToDbValue, UserId};

fn ts() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 22, 1, 2, 3).unwrap()
}

#[test]
fn str_ref_to_string_value() {
    let s: &str = "hello";
    assert!(matches!(s.to_db_value(), DbValue::String(v) if v == "hello"));
}

#[test]
fn owned_string_to_value() {
    let s = String::from("world");
    assert!(matches!(s.to_db_value(), DbValue::String(v) if v == "world"));
}

#[test]
fn string_ref_to_value() {
    let s = String::from("ref");
    let r: &String = &s;
    assert!(matches!(r.to_db_value(), DbValue::String(v) if v == "ref"));
}

#[test]
fn i32_and_ref_to_int() {
    let n: i32 = -7;
    assert!(matches!(n.to_db_value(), DbValue::Int(-7)));
    let r: &i32 = &n;
    assert!(matches!(r.to_db_value(), DbValue::Int(-7)));
}

#[test]
fn i64_and_ref_to_int() {
    let n: i64 = 9_000_000_000;
    assert!(matches!(n.to_db_value(), DbValue::Int(9_000_000_000)));
    let r: &i64 = &n;
    assert!(matches!(r.to_db_value(), DbValue::Int(9_000_000_000)));
}

#[test]
fn u32_to_int() {
    let n: u32 = 4_000_000_000;
    assert!(matches!(n.to_db_value(), DbValue::Int(v) if v == 4_000_000_000));
}

#[test]
fn u64_saturates_to_i64_max() {
    let n: u64 = u64::MAX;
    assert!(matches!(n.to_db_value(), DbValue::Int(v) if v == i64::MAX));
}

#[test]
fn u64_within_range_is_exact() {
    let n: u64 = 100;
    assert!(matches!(n.to_db_value(), DbValue::Int(100)));
}

#[test]
fn f32_widens_to_float() {
    let n: f32 = 1.5;
    assert!(matches!(n.to_db_value(), DbValue::Float(v) if (v - 1.5).abs() < f64::EPSILON));
}

#[test]
fn f64_and_ref_to_float() {
    let n: f64 = 2.5;
    assert!(matches!(n.to_db_value(), DbValue::Float(v) if (v - 2.5).abs() < f64::EPSILON));
    let r: &f64 = &n;
    assert!(matches!(r.to_db_value(), DbValue::Float(v) if (v - 2.5).abs() < f64::EPSILON));
}

#[test]
fn bool_and_ref_to_bool() {
    assert!(matches!(true.to_db_value(), DbValue::Bool(true)));
    let b = false;
    let r: &bool = &b;
    assert!(matches!(r.to_db_value(), DbValue::Bool(false)));
}

#[test]
fn bytes_vec_and_slice_to_bytes() {
    let v: Vec<u8> = vec![1, 2, 3];
    assert!(matches!(v.to_db_value(), DbValue::Bytes(b) if b == vec![1, 2, 3]));
    let slice: &[u8] = &[4, 5];
    assert!(matches!(slice.to_db_value(), DbValue::Bytes(b) if b == vec![4, 5]));
}

#[test]
fn datetime_and_ref_to_timestamp() {
    let t = ts();
    assert!(matches!(t.to_db_value(), DbValue::Timestamp(v) if v == t));
    let r: &DateTime<Utc> = &t;
    assert!(matches!(r.to_db_value(), DbValue::Timestamp(v) if v == t));
}

#[test]
fn string_vec_and_refs_to_string_array() {
    let v = vec!["a".to_owned(), "b".to_owned()];
    assert!(
        matches!(v.to_db_value(), DbValue::StringArray(a) if a == vec!["a".to_owned(), "b".to_owned()])
    );
    let r: &Vec<String> = &v;
    assert!(matches!(r.to_db_value(), DbValue::StringArray(a) if a.len() == 2));
    let slice: &[String] = &v;
    assert!(matches!(slice.to_db_value(), DbValue::StringArray(a) if a.len() == 2));
}

#[test]
fn option_some_delegates_to_inner() {
    let v: Option<i64> = Some(5);
    assert!(matches!(v.to_db_value(), DbValue::Int(5)));
}

#[test]
fn option_none_uses_typed_null() {
    let v: Option<i64> = None;
    assert!(matches!(v.to_db_value(), DbValue::NullInt));
    let s: Option<String> = None;
    assert!(matches!(s.to_db_value(), DbValue::NullString));
    let b: Option<bool> = None;
    assert!(matches!(b.to_db_value(), DbValue::NullBool));
    let f: Option<f64> = None;
    assert!(matches!(f.to_db_value(), DbValue::NullFloat));
    let bytes: Option<Vec<u8>> = None;
    assert!(matches!(bytes.to_db_value(), DbValue::NullBytes));
    let t: Option<DateTime<Utc>> = None;
    assert!(matches!(t.to_db_value(), DbValue::NullTimestamp));
    let arr: Option<Vec<String>> = None;
    assert!(matches!(arr.to_db_value(), DbValue::NullStringArray));
}

#[test]
fn typed_id_uses_default_null_db_value() {
    assert!(matches!(UserId::null_db_value(), DbValue::NullString));
}
