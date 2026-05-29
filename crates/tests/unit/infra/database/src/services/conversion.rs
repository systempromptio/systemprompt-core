//! Tests for `ToDbValue` / `DbValue` variants and `QueryResult` serialization paths.
//!
//! The `bind_params` / `row_to_json` functions require live sqlx rows and cannot
//! be exercised without a real pool. This file covers the `DbValue` enum surface
//! and the `ToDbValue` blanket impls from `systemprompt-traits`, which are the
//! pure-logic half of the conversion module.

use systemprompt_database::DbValue;

fn assert_db_value_debug(v: &DbValue) {
    let _ = format!("{:?}", v);
}

#[test]
fn db_value_string_variant() {
    let v = DbValue::String("hello".to_string());
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::String(_)));
}

#[test]
fn db_value_int_variant() {
    let v = DbValue::Int(42);
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::Int(42)));
}

#[test]
fn db_value_float_variant() {
    let v = DbValue::Float(3.14);
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::Float(_)));
}

#[test]
fn db_value_bool_true() {
    let v = DbValue::Bool(true);
    assert!(matches!(v, DbValue::Bool(true)));
}

#[test]
fn db_value_bool_false() {
    let v = DbValue::Bool(false);
    assert!(matches!(v, DbValue::Bool(false)));
}

#[test]
fn db_value_bytes_variant() {
    let v = DbValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::Bytes(_)));
}

#[test]
fn db_value_null_string() {
    let v = DbValue::NullString;
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::NullString));
}

#[test]
fn db_value_null_int() {
    let v = DbValue::NullInt;
    assert!(matches!(v, DbValue::NullInt));
}

#[test]
fn db_value_null_float() {
    let v = DbValue::NullFloat;
    assert!(matches!(v, DbValue::NullFloat));
}

#[test]
fn db_value_null_bool() {
    let v = DbValue::NullBool;
    assert!(matches!(v, DbValue::NullBool));
}

#[test]
fn db_value_null_bytes() {
    let v = DbValue::NullBytes;
    assert!(matches!(v, DbValue::NullBytes));
}

#[test]
fn db_value_null_timestamp() {
    let v = DbValue::NullTimestamp;
    assert!(matches!(v, DbValue::NullTimestamp));
}

#[test]
fn db_value_null_string_array() {
    let v = DbValue::NullStringArray;
    assert!(matches!(v, DbValue::NullStringArray));
}

#[test]
fn db_value_string_array_variant() {
    let v = DbValue::StringArray(vec!["a".to_string(), "b".to_string()]);
    assert_db_value_debug(&v);
    assert!(matches!(v, DbValue::StringArray(_)));
}

#[test]
fn to_db_value_for_str() {
    use systemprompt_database::ToDbValue;
    let val = "world".to_db_value();
    assert!(matches!(val, DbValue::String(s) if s == "world"));
}

#[test]
fn to_db_value_for_string() {
    use systemprompt_database::ToDbValue;
    let s = "owned".to_string();
    let val = s.to_db_value();
    assert!(matches!(val, DbValue::String(v) if v == "owned"));
}

#[test]
fn to_db_value_for_i32() {
    use systemprompt_database::ToDbValue;
    let val = 99i32.to_db_value();
    assert!(matches!(val, DbValue::Int(99)));
}

#[test]
fn to_db_value_for_i64() {
    use systemprompt_database::ToDbValue;
    let val = 12345i64.to_db_value();
    assert!(matches!(val, DbValue::Int(12345)));
}

#[test]
fn to_db_value_for_u32() {
    use systemprompt_database::ToDbValue;
    let val = 7u32.to_db_value();
    assert!(matches!(val, DbValue::Int(7)));
}

#[test]
fn to_db_value_for_bool_true() {
    use systemprompt_database::ToDbValue;
    let val = true.to_db_value();
    assert!(matches!(val, DbValue::Bool(true)));
}

#[test]
fn to_db_value_for_bool_false() {
    use systemprompt_database::ToDbValue;
    let val = false.to_db_value();
    assert!(matches!(val, DbValue::Bool(false)));
}

#[test]
fn to_db_value_for_option_some_str() {
    use systemprompt_database::ToDbValue;
    let opt: Option<&str> = Some("value");
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::String(s) if s == "value"));
}

#[test]
fn to_db_value_for_option_none_str() {
    use systemprompt_database::ToDbValue;
    let opt: Option<&str> = None;
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::NullString));
}

#[test]
fn to_db_value_for_option_some_i64() {
    use systemprompt_database::ToDbValue;
    let opt: Option<i64> = Some(55);
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::Int(55)));
}

#[test]
fn to_db_value_for_option_none_i64() {
    use systemprompt_database::ToDbValue;
    let opt: Option<i64> = None;
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::NullInt));
}

#[test]
fn to_db_value_for_option_some_bool() {
    use systemprompt_database::ToDbValue;
    let opt: Option<bool> = Some(false);
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::Bool(false)));
}

#[test]
fn to_db_value_for_option_none_bool() {
    use systemprompt_database::ToDbValue;
    let opt: Option<bool> = None;
    let val = opt.to_db_value();
    assert!(matches!(val, DbValue::NullBool));
}

#[test]
fn to_db_value_for_vec_string() {
    use systemprompt_database::ToDbValue;
    let arr = vec!["x".to_string(), "y".to_string()];
    let val = arr.to_db_value();
    assert!(matches!(val, DbValue::StringArray(_)));
}
