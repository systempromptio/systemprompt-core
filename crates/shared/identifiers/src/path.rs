//! Validated file path type.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct ValidatedFilePath(String);

impl ValidatedFilePath {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdValidationError::empty("ValidatedFilePath"));
        }
        if value.contains('\0') {
            return Err(IdValidationError::invalid(
                "ValidatedFilePath",
                "cannot contain null bytes",
            ));
        }
        for component in value.split(['/', '\\']) {
            if component == ".." {
                return Err(IdValidationError::invalid(
                    "ValidatedFilePath",
                    "cannot contain '..' path traversal",
                ));
            }
            let lower = component.to_lowercase();
            if lower.contains("%2e%2e") || lower.contains("%2e.") || lower.contains(".%2e") {
                return Err(IdValidationError::invalid(
                    "ValidatedFilePath",
                    "cannot contain encoded path traversal sequences",
                ));
            }
        }
        let lower_value = value.to_lowercase();
        if lower_value.contains("%252e") {
            return Err(IdValidationError::invalid(
                "ValidatedFilePath",
                "cannot contain double-encoded path sequences",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("ValidatedFilePath validation failed")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        self.0
            .rsplit('.')
            .next()
            .filter(|_| self.0.contains('.') && !self.0.ends_with('.'))
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.0.rsplit(['/', '\\']).next().filter(|s| !s.is_empty())
    }
}

impl fmt::Display for ValidatedFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for ValidatedFilePath {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for ValidatedFilePath {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for ValidatedFilePath {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for ValidatedFilePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ValidatedFilePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for ValidatedFilePath {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &ValidatedFilePath {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
