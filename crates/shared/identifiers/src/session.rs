//! Session identifier type.

use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Session identifier (always required for tracing and analytics)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for SessionId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &SessionId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
