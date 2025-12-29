//! Database value types.

use chrono::{DateTime, NaiveDateTime, Utc};
use std::collections::HashMap;

pub type JsonRow = HashMap<String, serde_json::Value>;

#[must_use]
pub fn parse_database_datetime(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    if let Some(s) = value.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Some(dt.with_timezone(&Utc));
        }

        if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
            return Some(dt.and_utc());
        }

        None
    } else if let Some(ts) = value.as_i64() {
        DateTime::from_timestamp(ts, 0)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub enum DbValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Bytes(Vec<u8>),
    Timestamp(DateTime<Utc>),
    StringArray(Vec<String>),
    NullString,
    NullInt,
    NullFloat,
    NullBool,
    NullBytes,
    NullTimestamp,
    NullStringArray,
}
