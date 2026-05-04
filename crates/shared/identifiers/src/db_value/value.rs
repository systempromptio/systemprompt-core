use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Map representation of a database row keyed by column name.
pub type JsonRow = HashMap<String, serde_json::Value>;

/// Parses a JSON value into a `DateTime<Utc>`, accepting either an RFC3339
/// string, a `YYYY-MM-DD HH:MM:SS[.fff]` string (UTC), or an integer Unix
/// timestamp in seconds.
#[must_use]
pub fn parse_database_datetime(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    if let Some(s) = value.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Some(dt.with_timezone(&Utc));
        }

        let with_tz = format!("{s}+00:00");
        if let Ok(dt) = DateTime::parse_from_str(&with_tz, "%Y-%m-%d %H:%M:%S%.f%:z") {
            return Some(dt.with_timezone(&Utc));
        }

        None
    } else if let Some(ts) = value.as_i64() {
        DateTime::from_timestamp(ts, 0)
    } else {
        None
    }
}

/// Tagged union of every scalar SQL value (and per-type NULL marker)
/// supported by the platform's repository layer.
#[derive(Debug, Clone)]
pub enum DbValue {
    /// `TEXT` / `VARCHAR` value.
    String(String),
    /// 64-bit signed integer value.
    Int(i64),
    /// 64-bit float value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Raw byte buffer (`BYTEA`).
    Bytes(Vec<u8>),
    /// `TIMESTAMPTZ` value.
    Timestamp(DateTime<Utc>),
    /// `TEXT[]` array value.
    StringArray(Vec<String>),
    /// `NULL` value typed as text.
    NullString,
    /// `NULL` value typed as integer.
    NullInt,
    /// `NULL` value typed as float.
    NullFloat,
    /// `NULL` value typed as boolean.
    NullBool,
    /// `NULL` value typed as bytes.
    NullBytes,
    /// `NULL` value typed as timestamp.
    NullTimestamp,
    /// `NULL` value typed as text array.
    NullStringArray,
}
