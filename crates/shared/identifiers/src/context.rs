//! Context identifier type.

use crate::{DbValue, ToDbValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct ContextId(String);

impl ContextId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub const fn empty() -> Self {
        Self(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ContextId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ContextId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ContextId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for ContextId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &ContextId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
