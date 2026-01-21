//! Agent identifier types.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for AgentId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &AgentId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    pub fn try_new(name: impl Into<String>) -> Result<Self, IdValidationError> {
        let name = name.into();
        if name.is_empty() {
            return Err(IdValidationError::empty("AgentName"));
        }
        if name.eq_ignore_ascii_case("unknown") {
            return Err(IdValidationError::invalid(
                "AgentName",
                "'unknown' is reserved for error detection",
            ));
        }
        Ok(Self(name))
    }

    #[allow(clippy::expect_used)]
    pub fn new(name: impl Into<String>) -> Self {
        Self::try_new(name).expect("AgentName validation failed")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }
}

impl AsRef<str> for AgentName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for AgentName {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for AgentName {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for AgentName {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl<'de> Deserialize<'de> for AgentName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for AgentName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &AgentName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
