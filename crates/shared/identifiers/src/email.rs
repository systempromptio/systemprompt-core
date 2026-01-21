//! Email identifier type with validation.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct Email(String);

impl Email {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdValidationError::empty("Email"));
        }
        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 {
            return Err(IdValidationError::invalid(
                "Email",
                "must contain exactly one '@' symbol",
            ));
        }
        let local = parts[0];
        let domain = parts[1];
        if local.is_empty() {
            return Err(IdValidationError::invalid(
                "Email",
                "local part (before @) cannot be empty",
            ));
        }
        // Local part validation
        if local.starts_with('.') || local.ends_with('.') {
            return Err(IdValidationError::invalid(
                "Email",
                "local part cannot start or end with '.'",
            ));
        }
        if local.contains("..") {
            return Err(IdValidationError::invalid(
                "Email",
                "local part cannot contain consecutive dots",
            ));
        }
        // Check for dangerous characters that could enable header injection
        if local.contains('\n') || local.contains('\r') {
            return Err(IdValidationError::invalid(
                "Email",
                "email cannot contain newline characters",
            ));
        }
        if domain.is_empty() {
            return Err(IdValidationError::invalid(
                "Email",
                "domain part (after @) cannot be empty",
            ));
        }
        if !domain.contains('.') {
            return Err(IdValidationError::invalid(
                "Email",
                "domain must contain at least one '.'",
            ));
        }
        if domain.starts_with('.') || domain.ends_with('.') {
            return Err(IdValidationError::invalid(
                "Email",
                "domain cannot start or end with '.'",
            ));
        }
        // Domain validation
        if domain.contains("..") {
            return Err(IdValidationError::invalid(
                "Email",
                "domain cannot contain consecutive dots",
            ));
        }
        // Check TLD has at least 2 characters
        if let Some(tld) = domain.rsplit('.').next() {
            if tld.len() < 2 {
                return Err(IdValidationError::invalid(
                    "Email",
                    "TLD must be at least 2 characters",
                ));
            }
        }
        Ok(Self(value))
    }

    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("Email validation failed")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn local_part(&self) -> &str {
        self.0.split('@').next().unwrap_or("")
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.split('@').nth(1).unwrap_or("")
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for Email {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for Email {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for Email {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for Email {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &Email {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
