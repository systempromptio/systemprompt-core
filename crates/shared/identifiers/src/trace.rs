//! Trace identifier type.

use crate::{DbValue, ToDbValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TraceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TraceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for TraceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for TraceId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &TraceId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
