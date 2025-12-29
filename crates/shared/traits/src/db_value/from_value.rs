use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

use super::{parse_database_datetime, DbValue};

pub trait FromDbValue: Sized {
    fn from_db_value(value: &DbValue) -> Result<Self>;
}

impl FromDbValue for String {
    fn from_db_value(value: &DbValue) -> Result<Self> {
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
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to String")),
            DbValue::Bytes(_) => Err(anyhow!("Cannot convert Bytes to String")),
        }
    }
}

impl FromDbValue for i64 {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        match value {
            DbValue::Int(i) => Ok(*i),
            DbValue::Float(f) => f64_to_i64_checked(*f),
            DbValue::Bool(b) => Ok(Self::from(*b)),
            DbValue::String(s) => s.parse().map_err(|_| anyhow!("Cannot parse {s} as i64")),
            DbValue::StringArray(_) => Err(anyhow!("Cannot convert StringArray to i64")),
            DbValue::Timestamp(_) => Err(anyhow!("Cannot convert Timestamp to i64")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to i64")),
            DbValue::Bytes(_) => Err(anyhow!("Cannot convert Bytes to i64")),
        }
    }
}

impl FromDbValue for i32 {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        i64::from_db_value(value).and_then(|v| {
            Self::try_from(v).map_err(|_| anyhow!("Integer overflow converting to i32"))
        })
    }
}

impl FromDbValue for u64 {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        i64::from_db_value(value).and_then(|v| {
            Self::try_from(v).map_err(|_| anyhow!("Negative value cannot convert to u64"))
        })
    }
}

impl FromDbValue for u32 {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        i64::from_db_value(value)
            .and_then(|v| Self::try_from(v).map_err(|_| anyhow!("Value out of range for u32")))
    }
}

const I64_MAX_SAFE_F64: i64 = 1 << 53;

fn i64_to_f64_checked(value: i64) -> Result<f64> {
    if value.abs() > I64_MAX_SAFE_F64 {
        return Err(anyhow!("Integer {value} exceeds f64 precision range"));
    }
    Ok(value as f64)
}

fn f64_to_i64_checked(value: f64) -> Result<i64> {
    if value.is_nan() || value.is_infinite() {
        return Err(anyhow!("Cannot convert NaN/Infinite to i64"));
    }
    if value < (i64::MIN as f64) || value > (i64::MAX as f64) {
        return Err(anyhow!("Float {value} out of range for i64"));
    }
    Ok(value as i64)
}

impl FromDbValue for f64 {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        match value {
            DbValue::Float(f) => Ok(*f),
            DbValue::Int(i) => i64_to_f64_checked(*i),
            DbValue::String(s) => s.parse().map_err(|_| anyhow!("Cannot parse {s} as f64")),
            DbValue::StringArray(_) => Err(anyhow!("Cannot convert StringArray to f64")),
            DbValue::Timestamp(_) => Err(anyhow!("Cannot convert Timestamp to f64")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to f64")),
            DbValue::Bool(_) => Err(anyhow!("Cannot convert Bool to f64")),
            DbValue::Bytes(_) => Err(anyhow!("Cannot convert Bytes to f64")),
        }
    }
}

impl FromDbValue for bool {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        match value {
            DbValue::Bool(b) => Ok(*b),
            DbValue::Int(i) => Ok(*i != 0),
            DbValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" => Ok(false),
                _ => Err(anyhow!("Cannot parse {s} as bool")),
            },
            DbValue::StringArray(_) => Err(anyhow!("Cannot convert StringArray to bool")),
            DbValue::Timestamp(_) => Err(anyhow!("Cannot convert Timestamp to bool")),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to bool")),
            DbValue::Float(_) => Err(anyhow!("Cannot convert Float to bool")),
            DbValue::Bytes(_) => Err(anyhow!("Cannot convert Bytes to bool")),
        }
    }
}

impl FromDbValue for Vec<u8> {
    fn from_db_value(value: &DbValue) -> Result<Self> {
        match value {
            DbValue::Bytes(b) => Ok(b.clone()),
            DbValue::String(s) => Ok(s.as_bytes().to_vec()),
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to Vec<u8>")),
            DbValue::Int(_)
            | DbValue::Float(_)
            | DbValue::Bool(_)
            | DbValue::Timestamp(_)
            | DbValue::StringArray(_) => Err(anyhow!("Cannot convert {value:?} to Vec<u8>")),
        }
    }
}

impl<T: FromDbValue> FromDbValue for Option<T> {
    fn from_db_value(value: &DbValue) -> Result<Self> {
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
    fn from_db_value(value: &DbValue) -> Result<Self> {
        match value {
            DbValue::String(s) => parse_database_datetime(&serde_json::Value::String(s.clone()))
                .ok_or_else(|| anyhow!("Cannot parse {s} as DateTime<Utc>")),
            DbValue::Timestamp(dt) => Ok(*dt),
            DbValue::Int(ts) => {
                Self::from_timestamp(*ts, 0).ok_or_else(|| anyhow!("Invalid Unix timestamp: {ts}"))
            },
            DbValue::NullString
            | DbValue::NullInt
            | DbValue::NullFloat
            | DbValue::NullBool
            | DbValue::NullBytes
            | DbValue::NullTimestamp
            | DbValue::NullStringArray => Err(anyhow!("Cannot convert NULL to DateTime<Utc>")),
            DbValue::Float(_) | DbValue::Bool(_) | DbValue::Bytes(_) | DbValue::StringArray(_) => {
                Err(anyhow!("Cannot convert {value:?} to DateTime<Utc>"))
            },
        }
    }
}
