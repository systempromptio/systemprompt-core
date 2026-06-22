//! Tests for `ToDbValue` / `DbValue` variants and `QueryResult` serialization
//! paths.
//!
//! The `bind_params` / `row_to_json` functions require live sqlx rows and
//! cannot be exercised without a real pool. This file covers the `DbValue` enum
//! surface and the `ToDbValue` blanket impls from `systemprompt-traits`, which
//! are the pure-logic half of the conversion module.

use super::db_helper::pool;
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

// --- DB-backed: row_to_json / column_to_json branch coverage. ---

#[tokio::test]
async fn row_to_json_converts_each_scalar_type() {
    let Some(db) = pool().await else { return };
    let provider = db.read();

    let sql = "SELECT \
        42::bigint        AS i64, \
        7::int            AS i32, \
        3.5::double precision AS f64, \
        1.25::numeric     AS num, \
        true              AS flag, \
        'hello'::text     AS txt, \
        '11111111-1111-1111-1111-111111111111'::uuid AS u, \
        '2020-01-02T03:04:05Z'::timestamptz AS ts, \
        ARRAY['a','b']::text[] AS arr, \
        '{\"k\":1}'::jsonb AS j, \
        '\\xDEADBEEF'::bytea AS bin, \
        NULL::text        AS nil";

    let result = provider.query_raw(&sql).await.expect("query_raw all types");
    assert_eq!(result.row_count, 1);
    let row = &result.rows[0];

    assert_eq!(row.get("i64").and_then(serde_json::Value::as_i64), Some(42));
    assert_eq!(row.get("i32").and_then(serde_json::Value::as_i64), Some(7));
    assert_eq!(
        row.get("f64").and_then(serde_json::Value::as_f64),
        Some(3.5)
    );
    assert_eq!(
        row.get("num").and_then(serde_json::Value::as_f64),
        Some(1.25)
    );
    assert_eq!(
        row.get("flag").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        row.get("txt").and_then(serde_json::Value::as_str),
        Some("hello")
    );
    assert_eq!(
        row.get("u").and_then(serde_json::Value::as_str),
        Some("11111111-1111-1111-1111-111111111111")
    );
    assert_eq!(
        row.get("ts").and_then(serde_json::Value::as_str),
        Some("2020-01-02T03:04:05+00:00")
    );

    let arr = row.get("arr").and_then(serde_json::Value::as_array);
    assert_eq!(
        arr.map(|a| a
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()),
        Some(vec!["a", "b"])
    );

    assert_eq!(
        row.get("j")
            .and_then(|v| v.get("k"))
            .and_then(serde_json::Value::as_i64),
        Some(1)
    );

    // bytea base64-encodes 0xDEADBEEF.
    assert_eq!(
        row.get("bin").and_then(serde_json::Value::as_str),
        Some("3q2+7w==")
    );

    assert_eq!(row.get("nil"), Some(&serde_json::Value::Null));
}

#[tokio::test]
async fn bind_params_round_trips_each_db_value_variant() {
    let Some(db) = pool().await else { return };
    let provider = db.read();

    let s = "bound".to_string();
    let i: i64 = 123;
    let f: f64 = 9.5;
    let b = true;
    let bytes: Vec<u8> = vec![1, 2, 3];
    let ts = chrono::DateTime::parse_from_rfc3339("2021-06-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let arr = vec!["p".to_string(), "q".to_string()];

    let params: Vec<&dyn systemprompt_database::ToDbValue> =
        vec![&s, &i, &f, &b, &bytes, &ts, &arr];

    let result = provider
        .query_raw_with(
            &"SELECT $1::text AS s, $2::bigint AS i, $3::double precision AS f, \
              $4::boolean AS b, $5::bytea AS by, $6::timestamptz AS t, $7::text[] AS a",
            &params,
        )
        .await
        .expect("query_raw_with bound params");

    let row = &result.rows[0];
    assert_eq!(
        row.get("s").and_then(serde_json::Value::as_str),
        Some("bound")
    );
    assert_eq!(row.get("i").and_then(serde_json::Value::as_i64), Some(123));
    assert_eq!(row.get("f").and_then(serde_json::Value::as_f64), Some(9.5));
    assert_eq!(
        row.get("b").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        row.get("t").and_then(serde_json::Value::as_str),
        Some("2021-06-01T00:00:00+00:00")
    );
    let a = row.get("a").and_then(serde_json::Value::as_array);
    assert_eq!(
        a.map(|v| v
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()),
        Some(vec!["p", "q"])
    );
}

#[tokio::test]
async fn bind_params_handles_null_variants() {
    let Some(db) = pool().await else { return };
    let provider = db.read();

    let null_string: Option<String> = None;
    let null_int: Option<i64> = None;
    let null_bool: Option<bool> = None;

    let params: Vec<&dyn systemprompt_database::ToDbValue> =
        vec![&null_string, &null_int, &null_bool];

    let result = provider
        .query_raw_with(
            &"SELECT $1::text AS s, $2::bigint AS i, $3::boolean AS b",
            &params,
        )
        .await
        .expect("query_raw_with null params");

    let row = &result.rows[0];
    assert_eq!(row.get("s"), Some(&serde_json::Value::Null));
    assert_eq!(row.get("i"), Some(&serde_json::Value::Null));
    assert_eq!(row.get("b"), Some(&serde_json::Value::Null));
}
