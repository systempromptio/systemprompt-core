//! Tests for DbValue type and parse_database_datetime function.

use chrono::{Datelike, TimeZone, Timelike, Utc};
use systemprompt_traits::{parse_database_datetime, DbValue};

mod parse_database_datetime_tests {
    use super::*;

    #[test]
    fn parses_rfc3339_string() {
        let value = serde_json::json!("2024-01-15T10:30:00Z");
        let result = parse_database_datetime(&value);

        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn parses_rfc3339_with_timezone_offset() {
        let value = serde_json::json!("2024-06-20T15:45:30+05:00");
        let result = parse_database_datetime(&value);

        assert!(result.is_some());
        let dt = result.unwrap();
        // Should be converted to UTC (15:45 +05:00 = 10:45 UTC)
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn parses_database_format_without_timezone() {
        let value = serde_json::json!("2024-03-10 14:25:30.123");
        let result = parse_database_datetime(&value);

        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 3);
        assert_eq!(dt.day(), 10);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 25);
        assert_eq!(dt.second(), 30);
    }

    #[test]
    fn parses_unix_timestamp() {
        let value = serde_json::json!(1705318200_i64); // 2024-01-15T10:30:00Z
        let result = parse_database_datetime(&value);

        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
    }

    #[test]
    fn returns_none_for_invalid_string() {
        let value = serde_json::json!("not a date");
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_null() {
        let value = serde_json::Value::Null;
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_object() {
        let value = serde_json::json!({"date": "2024-01-15"});
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_array() {
        let value = serde_json::json!([2024, 1, 15]);
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_boolean() {
        let value = serde_json::json!(true);
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_float() {
        let value = serde_json::json!(1705318200.5);
        let result = parse_database_datetime(&value);

        assert!(result.is_none());
    }
}

mod db_value_enum_tests {
    use super::*;

    #[test]
    fn can_create_string_value() {
        let value = DbValue::String("hello".to_string());
        assert!(matches!(value, DbValue::String(s) if s == "hello"));
    }

    #[test]
    fn can_create_int_value() {
        let value = DbValue::Int(42);
        assert!(matches!(value, DbValue::Int(42)));
    }

    #[test]
    fn can_create_float_value() {
        let value = DbValue::Float(3.14);
        assert!(matches!(value, DbValue::Float(f) if (f - 3.14).abs() < f64::EPSILON));
    }

    #[test]
    fn can_create_bool_value() {
        let value_true = DbValue::Bool(true);
        let value_false = DbValue::Bool(false);
        assert!(matches!(value_true, DbValue::Bool(true)));
        assert!(matches!(value_false, DbValue::Bool(false)));
    }

    #[test]
    fn can_create_bytes_value() {
        let value = DbValue::Bytes(vec![1, 2, 3, 4]);
        assert!(matches!(value, DbValue::Bytes(b) if b == vec![1, 2, 3, 4]));
    }

    #[test]
    fn can_create_timestamp_value() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let value = DbValue::Timestamp(dt);
        assert!(matches!(value, DbValue::Timestamp(t) if t.year() == 2024));
    }

    #[test]
    fn can_create_string_array_value() {
        let value = DbValue::StringArray(vec!["a".to_string(), "b".to_string()]);
        assert!(matches!(value, DbValue::StringArray(arr) if arr.len() == 2));
    }

    #[test]
    fn can_create_null_variants() {
        let _ = DbValue::NullString;
        let _ = DbValue::NullInt;
        let _ = DbValue::NullFloat;
        let _ = DbValue::NullBool;
        let _ = DbValue::NullBytes;
        let _ = DbValue::NullTimestamp;
        let _ = DbValue::NullStringArray;
    }

    #[test]
    fn db_value_is_clone() {
        let value = DbValue::String("test".to_string());
        let cloned = value.clone();
        assert!(matches!(cloned, DbValue::String(s) if s == "test"));
    }

    #[test]
    fn db_value_is_debug() {
        let value = DbValue::Int(123);
        let debug_str = format!("{:?}", value);
        assert!(debug_str.contains("Int"));
        assert!(debug_str.contains("123"));
    }
}
