//! User identifier type.

use crate::{DbValue, ToDbValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn anonymous() -> Self {
        Self("anonymous".to_string())
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for UserId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for UserId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for UserId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &UserId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
