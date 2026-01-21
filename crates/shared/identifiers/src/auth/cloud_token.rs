//! Cloud authentication token identifier type.

use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct CloudAuthToken(String);

impl CloudAuthToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn redacted(&self) -> String {
        let len = self.0.len();
        if len <= 16 {
            "*".repeat(len.min(8))
        } else {
            format!("{}...{}", &self.0[..8], &self.0[len - 4..])
        }
    }
}

impl fmt::Display for CloudAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.redacted())
    }
}

impl From<String> for CloudAuthToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CloudAuthToken {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for CloudAuthToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for CloudAuthToken {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &CloudAuthToken {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
