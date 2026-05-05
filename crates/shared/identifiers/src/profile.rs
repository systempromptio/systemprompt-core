//! Profile name identifier type with validation.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct ProfileName(String);

impl ProfileName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdValidationError::empty("ProfileName"));
        }
        if value.contains('/') {
            return Err(IdValidationError::invalid(
                "ProfileName",
                "cannot contain path separator '/'",
            ));
        }
        if !value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(IdValidationError::invalid(
                "ProfileName",
                "can only contain alphanumeric characters, hyphens, and underscores",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("ProfileName validation failed")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn default_profile() -> Self {
        Self("default".to_string())
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for ProfileName {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for ProfileName {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for ProfileName {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for ProfileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for ProfileName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &ProfileName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
