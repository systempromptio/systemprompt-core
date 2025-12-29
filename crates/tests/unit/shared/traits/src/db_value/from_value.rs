//! Tests for FromDbValue trait implementations.

use chrono::{DateTime, Datelike, TimeZone, Utc};
use systemprompt_traits::{DbValue, FromDbValue};

mod string_from_db_value_tests {
    use super::*;

    #[test]
    fn from_string_value() {
        let value = DbValue::String("hello".to_string());
        let result = String::from_db_value(&value).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn from_int_value() {
        let value = DbValue::Int(42);
        let result = String::from_db_value(&value).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn from_float_value() {
        let value = DbValue::Float(3.14);
        let result = String::from_db_value(&value).unwrap();
        assert!(result.starts_with("3.14"));
    }

    #[test]
    fn from_bool_value() {
        let value_true = DbValue::Bool(true);
        let value_false = DbValue::Bool(false);
        assert_eq!(String::from_db_value(&value_true).unwrap(), "true");
        assert_eq!(String::from_db_value(&value_false).unwrap(), "false");
    }

    #[test]
    fn from_timestamp_value() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let value = DbValue::Timestamp(dt);
        let result = String::from_db_value(&value).unwrap();
        assert!(result.contains("2024"));
    }

    #[test]
    fn from_string_array_value() {
        let value = DbValue::StringArray(vec!["a".to_string(), "b".to_string()]);
        let result = String::from_db_value(&value).unwrap();
        assert!(result.contains("a"));
        assert!(result.contains("b"));
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullString;
        let result = String::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_bytes_fails() {
        let value = DbValue::Bytes(vec![1, 2, 3]);
        let result = String::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod i64_from_db_value_tests {
    use super::*;

    #[test]
    fn from_int_value() {
        let value = DbValue::Int(123);
        let result = i64::from_db_value(&value).unwrap();
        assert_eq!(result, 123);
    }

    #[test]
    fn from_float_value() {
        let value = DbValue::Float(42.0);
        let result = i64::from_db_value(&value).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn from_bool_value() {
        let value_true = DbValue::Bool(true);
        let value_false = DbValue::Bool(false);
        assert_eq!(i64::from_db_value(&value_true).unwrap(), 1);
        assert_eq!(i64::from_db_value(&value_false).unwrap(), 0);
    }

    #[test]
    fn from_string_value() {
        let value = DbValue::String("456".to_string());
        let result = i64::from_db_value(&value).unwrap();
        assert_eq!(result, 456);
    }

    #[test]
    fn from_invalid_string_fails() {
        let value = DbValue::String("not a number".to_string());
        let result = i64::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullInt;
        let result = i64::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_nan_fails() {
        let value = DbValue::Float(f64::NAN);
        let result = i64::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_infinity_fails() {
        let value = DbValue::Float(f64::INFINITY);
        let result = i64::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod i32_from_db_value_tests {
    use super::*;

    #[test]
    fn from_int_value() {
        let value = DbValue::Int(100);
        let result = i32::from_db_value(&value).unwrap();
        assert_eq!(result, 100);
    }

    #[test]
    fn overflow_fails() {
        let value = DbValue::Int(i64::MAX);
        let result = i32::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod u64_from_db_value_tests {
    use super::*;

    #[test]
    fn from_positive_int() {
        let value = DbValue::Int(500);
        let result = u64::from_db_value(&value).unwrap();
        assert_eq!(result, 500);
    }

    #[test]
    fn from_negative_fails() {
        let value = DbValue::Int(-1);
        let result = u64::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod u32_from_db_value_tests {
    use super::*;

    #[test]
    fn from_int_value() {
        let value = DbValue::Int(250);
        let result = u32::from_db_value(&value).unwrap();
        assert_eq!(result, 250);
    }

    #[test]
    fn from_negative_fails() {
        let value = DbValue::Int(-100);
        let result = u32::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn overflow_fails() {
        let value = DbValue::Int(i64::from(u32::MAX) + 1);
        let result = u32::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod f64_from_db_value_tests {
    use super::*;

    #[test]
    fn from_float_value() {
        let value = DbValue::Float(3.14159);
        let result = f64::from_db_value(&value).unwrap();
        assert!((result - 3.14159).abs() < f64::EPSILON);
    }

    #[test]
    fn from_int_value() {
        let value = DbValue::Int(42);
        let result = f64::from_db_value(&value).unwrap();
        assert!((result - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn from_string_value() {
        let value = DbValue::String("2.718".to_string());
        let result = f64::from_db_value(&value).unwrap();
        assert!((result - 2.718).abs() < 0.001);
    }

    #[test]
    fn from_invalid_string_fails() {
        let value = DbValue::String("not a float".to_string());
        let result = f64::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullFloat;
        let result = f64::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_bool_fails() {
        let value = DbValue::Bool(true);
        let result = f64::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod bool_from_db_value_tests {
    use super::*;

    #[test]
    fn from_bool_value() {
        let value_true = DbValue::Bool(true);
        let value_false = DbValue::Bool(false);
        assert!(bool::from_db_value(&value_true).unwrap());
        assert!(!bool::from_db_value(&value_false).unwrap());
    }

    #[test]
    fn from_int_value() {
        let value_one = DbValue::Int(1);
        let value_zero = DbValue::Int(0);
        let value_other = DbValue::Int(42);
        assert!(bool::from_db_value(&value_one).unwrap());
        assert!(!bool::from_db_value(&value_zero).unwrap());
        assert!(bool::from_db_value(&value_other).unwrap());
    }

    #[test]
    fn from_string_true_values() {
        for s in &["true", "TRUE", "True", "1", "yes", "YES", "Yes"] {
            let value = DbValue::String((*s).to_string());
            assert!(bool::from_db_value(&value).unwrap(), "Failed for: {}", s);
        }
    }

    #[test]
    fn from_string_false_values() {
        for s in &["false", "FALSE", "False", "0", "no", "NO", "No"] {
            let value = DbValue::String((*s).to_string());
            assert!(!bool::from_db_value(&value).unwrap(), "Failed for: {}", s);
        }
    }

    #[test]
    fn from_invalid_string_fails() {
        let value = DbValue::String("maybe".to_string());
        let result = bool::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullBool;
        let result = bool::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_float_fails() {
        let value = DbValue::Float(1.0);
        let result = bool::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod vec_u8_from_db_value_tests {
    use super::*;

    #[test]
    fn from_bytes_value() {
        let value = DbValue::Bytes(vec![1, 2, 3, 4]);
        let result = Vec::<u8>::from_db_value(&value).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn from_string_value() {
        let value = DbValue::String("hello".to_string());
        let result = Vec::<u8>::from_db_value(&value).unwrap();
        assert_eq!(result, b"hello".to_vec());
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullBytes;
        let result = Vec::<u8>::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_int_fails() {
        let value = DbValue::Int(123);
        let result = Vec::<u8>::from_db_value(&value);
        assert!(result.is_err());
    }
}

mod option_from_db_value_tests {
    use super::*;

    #[test]
    fn some_from_string_value() {
        let value = DbValue::String("test".to_string());
        let result = Option::<String>::from_db_value(&value).unwrap();
        assert_eq!(result, Some("test".to_string()));
    }

    #[test]
    fn none_from_null_string() {
        let value = DbValue::NullString;
        let result = Option::<String>::from_db_value(&value).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn some_from_int_value() {
        let value = DbValue::Int(42);
        let result = Option::<i64>::from_db_value(&value).unwrap();
        assert_eq!(result, Some(42));
    }

    #[test]
    fn none_from_null_int() {
        let value = DbValue::NullInt;
        let result = Option::<i64>::from_db_value(&value).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn none_from_any_null_variant() {
        assert!(Option::<String>::from_db_value(&DbValue::NullFloat).unwrap().is_none());
        assert!(Option::<i64>::from_db_value(&DbValue::NullBool).unwrap().is_none());
        assert!(Option::<bool>::from_db_value(&DbValue::NullBytes).unwrap().is_none());
    }
}

mod datetime_from_db_value_tests {
    use super::*;

    #[test]
    fn from_timestamp_value() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let value = DbValue::Timestamp(dt);
        let result = DateTime::<Utc>::from_db_value(&value).unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 1);
        assert_eq!(result.day(), 15);
    }

    #[test]
    fn from_rfc3339_string() {
        let value = DbValue::String("2024-06-20T15:30:00Z".to_string());
        let result = DateTime::<Utc>::from_db_value(&value).unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 6);
    }

    #[test]
    fn from_database_format_string() {
        let value = DbValue::String("2024-03-10 14:25:30.123".to_string());
        let result = DateTime::<Utc>::from_db_value(&value).unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 3);
    }

    #[test]
    fn from_unix_timestamp() {
        let value = DbValue::Int(1705318200); // 2024-01-15T10:30:00Z
        let result = DateTime::<Utc>::from_db_value(&value).unwrap();
        assert_eq!(result.year(), 2024);
    }

    #[test]
    fn from_invalid_string_fails() {
        let value = DbValue::String("not a date".to_string());
        let result = DateTime::<Utc>::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_null_fails() {
        let value = DbValue::NullTimestamp;
        let result = DateTime::<Utc>::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_float_fails() {
        let value = DbValue::Float(1705318200.5);
        let result = DateTime::<Utc>::from_db_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn from_bool_fails() {
        let value = DbValue::Bool(true);
        let result = DateTime::<Utc>::from_db_value(&value);
        assert!(result.is_err());
    }
}
