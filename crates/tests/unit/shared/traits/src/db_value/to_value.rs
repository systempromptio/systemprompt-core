//! Tests for ToDbValue trait implementations.

use chrono::{Datelike, TimeZone, Utc};
use systemprompt_traits::{DbValue, ToDbValue};

mod string_to_db_value_tests {
    use super::*;

    #[test]
    fn str_to_db_value() {
        let s = "hello";
        let value = s.to_db_value();
        assert!(matches!(value, DbValue::String(v) if v == "hello"));
    }

    #[test]
    fn string_to_db_value() {
        let s = String::from("world");
        let value = s.to_db_value();
        assert!(matches!(value, DbValue::String(v) if v == "world"));
    }

    #[test]
    fn string_ref_to_db_value() {
        let s = String::from("test");
        let value = (&s).to_db_value();
        assert!(matches!(value, DbValue::String(v) if v == "test"));
    }

    #[test]
    fn str_null_db_value() {
        let null = <&str>::null_db_value();
        assert!(matches!(null, DbValue::NullString));
    }

    #[test]
    fn string_null_db_value() {
        let null = String::null_db_value();
        assert!(matches!(null, DbValue::NullString));
    }
}

mod integer_to_db_value_tests {
    use super::*;

    #[test]
    fn i32_to_db_value() {
        let n: i32 = 42;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Int(42)));
    }

    #[test]
    fn i64_to_db_value() {
        let n: i64 = 9_000_000_000;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Int(9_000_000_000)));
    }

    #[test]
    fn u32_to_db_value() {
        let n: u32 = 100;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Int(100)));
    }

    #[test]
    fn u64_to_db_value() {
        let n: u64 = 500;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Int(500)));
    }

    #[test]
    fn u64_large_value_clamps_to_i64_max() {
        let n: u64 = u64::MAX;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Int(i64::MAX)));
    }

    #[test]
    fn i32_ref_to_db_value() {
        let n: i32 = 123;
        let value = (&n).to_db_value();
        assert!(matches!(value, DbValue::Int(123)));
    }

    #[test]
    fn i64_ref_to_db_value() {
        let n: i64 = 456;
        let value = (&n).to_db_value();
        assert!(matches!(value, DbValue::Int(456)));
    }

    #[test]
    fn integer_null_db_value() {
        assert!(matches!(i32::null_db_value(), DbValue::NullInt));
        assert!(matches!(i64::null_db_value(), DbValue::NullInt));
        assert!(matches!(u32::null_db_value(), DbValue::NullInt));
        assert!(matches!(u64::null_db_value(), DbValue::NullInt));
    }
}

mod float_to_db_value_tests {
    use super::*;

    #[test]
    fn f32_to_db_value() {
        let n: f32 = 3.14;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Float(f) if (f - 3.14_f64).abs() < 0.01));
    }

    #[test]
    fn f64_to_db_value() {
        let n: f64 = 2.718281828;
        let value = n.to_db_value();
        assert!(matches!(value, DbValue::Float(f) if (f - 2.718281828).abs() < f64::EPSILON));
    }

    #[test]
    fn f64_ref_to_db_value() {
        let n: f64 = 1.5;
        let value = (&n).to_db_value();
        assert!(matches!(value, DbValue::Float(f) if (f - 1.5).abs() < f64::EPSILON));
    }

    #[test]
    fn float_null_db_value() {
        assert!(matches!(f32::null_db_value(), DbValue::NullFloat));
        assert!(matches!(f64::null_db_value(), DbValue::NullFloat));
    }
}

mod bool_to_db_value_tests {
    use super::*;

    #[test]
    fn bool_true_to_db_value() {
        let value = true.to_db_value();
        assert!(matches!(value, DbValue::Bool(true)));
    }

    #[test]
    fn bool_false_to_db_value() {
        let value = false.to_db_value();
        assert!(matches!(value, DbValue::Bool(false)));
    }

    #[test]
    fn bool_ref_to_db_value() {
        let b = true;
        let value = (&b).to_db_value();
        assert!(matches!(value, DbValue::Bool(true)));
    }

    #[test]
    fn bool_null_db_value() {
        assert!(matches!(bool::null_db_value(), DbValue::NullBool));
    }
}

mod bytes_to_db_value_tests {
    use super::*;

    #[test]
    fn vec_u8_to_db_value() {
        let bytes = vec![1_u8, 2, 3, 4];
        let value = bytes.to_db_value();
        assert!(matches!(value, DbValue::Bytes(b) if b == vec![1, 2, 3, 4]));
    }

    #[test]
    fn slice_u8_to_db_value() {
        let bytes: &[u8] = &[5, 6, 7, 8];
        let value = bytes.to_db_value();
        assert!(matches!(value, DbValue::Bytes(b) if b == vec![5, 6, 7, 8]));
    }

    #[test]
    fn bytes_null_db_value() {
        assert!(matches!(Vec::<u8>::null_db_value(), DbValue::NullBytes));
        assert!(matches!(<&[u8]>::null_db_value(), DbValue::NullBytes));
    }
}

mod option_to_db_value_tests {
    use super::*;

    #[test]
    fn some_string_to_db_value() {
        let opt: Option<String> = Some("hello".to_string());
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::String(s) if s == "hello"));
    }

    #[test]
    fn none_string_to_db_value() {
        let opt: Option<String> = None;
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::NullString));
    }

    #[test]
    fn some_i64_to_db_value() {
        let opt: Option<i64> = Some(42);
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::Int(42)));
    }

    #[test]
    fn none_i64_to_db_value() {
        let opt: Option<i64> = None;
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::NullInt));
    }

    #[test]
    fn some_bool_to_db_value() {
        let opt: Option<bool> = Some(true);
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::Bool(true)));
    }

    #[test]
    fn none_bool_to_db_value() {
        let opt: Option<bool> = None;
        let value = opt.to_db_value();
        assert!(matches!(value, DbValue::NullBool));
    }
}

mod datetime_to_db_value_tests {
    use super::*;

    #[test]
    fn datetime_to_db_value() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let value = dt.to_db_value();
        assert!(matches!(value, DbValue::Timestamp(t) if t.year() == 2024));
    }

    #[test]
    fn datetime_ref_to_db_value() {
        let dt = Utc.with_ymd_and_hms(2024, 6, 20, 15, 0, 0).unwrap();
        let value = (&dt).to_db_value();
        assert!(matches!(value, DbValue::Timestamp(t) if t.month() == 6));
    }

    #[test]
    fn datetime_null_db_value() {
        use chrono::DateTime;
        assert!(matches!(DateTime::<Utc>::null_db_value(), DbValue::NullTimestamp));
    }
}

mod string_array_to_db_value_tests {
    use super::*;

    #[test]
    fn vec_string_to_db_value() {
        let arr = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let value = arr.to_db_value();
        assert!(matches!(value, DbValue::StringArray(a) if a.len() == 3));
    }

    #[test]
    fn vec_string_ref_to_db_value() {
        let arr = vec!["x".to_string(), "y".to_string()];
        let value = (&arr).to_db_value();
        assert!(matches!(value, DbValue::StringArray(a) if a.len() == 2));
    }

    #[test]
    fn slice_string_to_db_value() {
        let arr = vec!["one".to_string(), "two".to_string()];
        let slice: &[String] = &arr;
        let value = slice.to_db_value();
        assert!(matches!(value, DbValue::StringArray(a) if a.len() == 2));
    }

    #[test]
    fn string_array_null_db_value() {
        assert!(matches!(Vec::<String>::null_db_value(), DbValue::NullStringArray));
        assert!(matches!(<&[String]>::null_db_value(), DbValue::NullStringArray));
    }
}
