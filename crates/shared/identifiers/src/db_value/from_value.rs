use chrono::{DateTime, Utc};

use super::{DbValue, parse_database_datetime};
use crate::error::DbValueError;

pub trait FromDbValue: Sized {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError>;
}

impl FromDbValue for String {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::String(s) => Ok(s.clone()),
            DbValue::Int(i) => Ok(i.to_string()),
            DbValue::Float(f) => Ok(f.to_string()),
            DbValue::Bool(b) => Ok(b.to_string()),
            DbValue::Timestamp(dt) => Ok(dt.to_rfc3339()),
            DbValue::StringArray(arr) => {
                Ok(serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string()))
            },
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("String")),
            DbValue::Bytes(_) => Err(DbValueError::incompatible("Bytes", "String")),
        }
    }
}

impl FromDbValue for i64 {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::Int(i) => Ok(*i),
            DbValue::Float(f) => f64_to_i64_checked(*f),
            DbValue::Bool(b) => Ok(Self::from(*b)),
            DbValue::String(s) => s.parse().map_err(|_| DbValueError::parse(s.clone(), "i64")),
            DbValue::StringArray(_) => Err(DbValueError::incompatible("StringArray", "i64")),
            DbValue::Timestamp(_) => Err(DbValueError::incompatible("Timestamp", "i64")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("i64")),
            DbValue::Bytes(_) => Err(DbValueError::incompatible("Bytes", "i64")),
        }
    }
}

impl FromDbValue for i32 {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        i64::from_db_value(value)
            .and_then(|v| Self::try_from(v).map_err(|_| DbValueError::out_of_range("i32")))
    }
}

impl FromDbValue for u64 {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        i64::from_db_value(value)
            .and_then(|v| Self::try_from(v).map_err(|_| DbValueError::out_of_range("u64")))
    }
}

impl FromDbValue for u32 {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        i64::from_db_value(value)
            .and_then(|v| Self::try_from(v).map_err(|_| DbValueError::out_of_range("u32")))
    }
}

const I64_MAX_SAFE_F64: i64 = 1 << 53;

const fn i64_to_f64_checked(value: i64) -> Result<f64, DbValueError> {
    if value.abs() > I64_MAX_SAFE_F64 {
        return Err(DbValueError::out_of_range("f64"));
    }
    Ok(value as f64)
}

fn f64_to_i64_checked(value: f64) -> Result<i64, DbValueError> {
    if value.is_nan() || value.is_infinite() {
        return Err(DbValueError::incompatible("NaN/Infinite", "i64"));
    }
    if value < (i64::MIN as f64) || value > (i64::MAX as f64) {
        return Err(DbValueError::out_of_range("i64"));
    }
    Ok(value as i64)
}

impl FromDbValue for f64 {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::Float(f) => Ok(*f),
            DbValue::Int(i) => i64_to_f64_checked(*i),
            DbValue::String(s) => s.parse().map_err(|_| DbValueError::parse(s.clone(), "f64")),
            DbValue::StringArray(_) => Err(DbValueError::incompatible("StringArray", "f64")),
            DbValue::Timestamp(_) => Err(DbValueError::incompatible("Timestamp", "f64")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("f64")),
            DbValue::Bool(_) => Err(DbValueError::incompatible("Bool", "f64")),
            DbValue::Bytes(_) => Err(DbValueError::incompatible("Bytes", "f64")),
        }
    }
}

impl FromDbValue for bool {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::Bool(b) => Ok(*b),
            DbValue::Int(i) => Ok(*i != 0),
            DbValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" => Ok(false),
                _ => Err(DbValueError::parse(s.clone(), "bool")),
            },
            DbValue::StringArray(_) => Err(DbValueError::incompatible("StringArray", "bool")),
            DbValue::Timestamp(_) => Err(DbValueError::incompatible("Timestamp", "bool")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("bool")),
            DbValue::Float(_) => Err(DbValueError::incompatible("Float", "bool")),
            DbValue::Bytes(_) => Err(DbValueError::incompatible("Bytes", "bool")),
        }
    }
}

impl FromDbValue for Vec<u8> {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::Bytes(b) => Ok(b.clone()),
            DbValue::String(s) => Ok(s.as_bytes().to_vec()),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("Vec<u8>")),
            DbValue::Int(_)
            | DbValue::Float(_)
            | DbValue::Bool(_)
            | DbValue::Timestamp(_)
            | DbValue::StringArray(_) => Err(DbValueError::incompatible("non-bytes", "Vec<u8>")),
        }
    }
}

impl<T: FromDbValue> FromDbValue for Option<T> {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Ok(None),
            DbValue::String(_)
            | DbValue::Int(_)
            | DbValue::Float(_)
            | DbValue::Bool(_)
            | DbValue::Bytes(_)
            | DbValue::Timestamp(_)
            | DbValue::StringArray(_) => T::from_db_value(value).map(Some),
        }
    }
}

impl FromDbValue for DateTime<Utc> {
    fn from_db_value(value: &DbValue) -> Result<Self, DbValueError> {
        match value {
            DbValue::String(s) => parse_database_datetime(&serde_json::Value::String(s.clone()))
                .ok_or_else(|| DbValueError::parse(s.clone(), "DateTime<Utc>")),
            DbValue::Timestamp(dt) => Ok(*dt),
            DbValue::Int(ts) => Self::from_timestamp(*ts, 0)
                .ok_or_else(|| DbValueError::parse(ts.to_string(), "DateTime<Utc>")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(DbValueError::null_for("DateTime<Utc>")),
            DbValue::Float(_) | DbValue::Bool(_) | DbValue::Bytes(_) | DbValue::StringArray(_) => {
                Err(DbValueError::incompatible("non-datetime", "DateTime<Utc>"))
            },
        }
    }
}
