//! Role identifier types.

use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct RoleId(String);

impl RoleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for RoleId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RoleId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for RoleId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for RoleId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &RoleId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
