//! Job-related identifier types.

use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct ScheduledJobId(String);

impl ScheduledJobId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScheduledJobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ScheduledJobId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ScheduledJobId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ScheduledJobId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for ScheduledJobId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &ScheduledJobId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct JobName(String);

impl JobName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for JobName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for JobName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for JobName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for JobName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &JobName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
