use chrono::{DateTime, Utc};

use super::DbValue;

/// Conversion from a Rust value into a [`DbValue`] container so it can be
/// bound to a SQL statement parameter.
///
/// Implemented for the canonical scalar types plus their borrowed forms,
/// `Option<T>`, `Vec<u8>`/`&[u8]`, `Vec<String>`/`&[String]`, and
/// `DateTime<Utc>`.
///
/// Custom identifier newtypes implement this via the
/// `__define_id_common!` helper macro.
pub trait ToDbValue: Send + Sync {
    /// Converts `self` into the appropriate [`DbValue`] variant.
    fn to_db_value(&self) -> DbValue;

    /// Returns the typed-NULL marker for this value type.
    #[must_use]
    fn null_db_value() -> DbValue
    where
        Self: Sized,
    {
        DbValue::NullString
    }
}

impl ToDbValue for &str {
    fn to_db_value(&self) -> DbValue {
        DbValue::String((*self).to_string())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullString
    }
}

impl ToDbValue for String {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.clone())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullString
    }
}

impl ToDbValue for &String {
    fn to_db_value(&self) -> DbValue {
        DbValue::String((*self).clone())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullString
    }
}

impl ToDbValue for i32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(i64::from(*self))
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for i64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(*self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for u32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(i64::from(*self))
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for u64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(i64::try_from(*self).unwrap_or(i64::MAX))
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for f32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Float(f64::from(*self))
    }

    fn null_db_value() -> DbValue {
        DbValue::NullFloat
    }
}

impl ToDbValue for f64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Float(*self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullFloat
    }
}

impl ToDbValue for &f64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Float(**self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullFloat
    }
}

impl ToDbValue for &i32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(i64::from(**self))
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for &i64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Int(**self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullInt
    }
}

impl ToDbValue for &bool {
    fn to_db_value(&self) -> DbValue {
        DbValue::Bool(**self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullBool
    }
}

impl ToDbValue for bool {
    fn to_db_value(&self) -> DbValue {
        DbValue::Bool(*self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullBool
    }
}

impl ToDbValue for Vec<u8> {
    fn to_db_value(&self) -> DbValue {
        DbValue::Bytes(self.clone())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullBytes
    }
}

impl ToDbValue for &[u8] {
    fn to_db_value(&self) -> DbValue {
        DbValue::Bytes(self.to_vec())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullBytes
    }
}

impl<T: ToDbValue> ToDbValue for Option<T> {
    fn to_db_value(&self) -> DbValue {
        self.as_ref()
            .map_or_else(T::null_db_value, ToDbValue::to_db_value)
    }
}

impl ToDbValue for DateTime<Utc> {
    fn to_db_value(&self) -> DbValue {
        DbValue::Timestamp(*self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullTimestamp
    }
}

impl ToDbValue for &DateTime<Utc> {
    fn to_db_value(&self) -> DbValue {
        DbValue::Timestamp(**self)
    }

    fn null_db_value() -> DbValue {
        DbValue::NullTimestamp
    }
}

impl ToDbValue for Vec<String> {
    fn to_db_value(&self) -> DbValue {
        DbValue::StringArray(self.clone())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullStringArray
    }
}

impl ToDbValue for &Vec<String> {
    fn to_db_value(&self) -> DbValue {
        DbValue::StringArray((*self).clone())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullStringArray
    }
}

impl ToDbValue for &[String] {
    fn to_db_value(&self) -> DbValue {
        DbValue::StringArray(self.to_vec())
    }

    fn null_db_value() -> DbValue {
        DbValue::NullStringArray
    }
}
