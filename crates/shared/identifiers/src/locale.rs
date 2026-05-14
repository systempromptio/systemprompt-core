//! BCP-47-lite locale identifier.
//!
//! Accepts the common subset of language tags: a 2- or 3-letter primary
//! subtag, optionally followed by additional `-`-separated subtags of 2-8
//! alphanumerics each. Total length capped at 35 characters per RFC 5646.
//! Comparison is case-sensitive on the lowercase primary subtag; callers
//! that need full BCP-47 canonicalisation should add the `language-tags`
//! crate when the requirement arrives.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_LEN: usize = 35;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(transparent))]
#[serde(transparent)]
pub struct LocaleCode(String);

impl LocaleCode {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdValidationError::empty("LocaleCode"));
        }
        if value.len() > MAX_LEN {
            return Err(IdValidationError::invalid(
                "LocaleCode",
                "exceeds 35 characters",
            ));
        }
        let mut subtags = value.split('-');
        let primary = subtags.next().unwrap_or("");
        let plen = primary.len();
        if !(2..=3).contains(&plen) || !primary.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(IdValidationError::invalid(
                "LocaleCode",
                "primary subtag must be 2-3 lowercase ASCII letters",
            ));
        }
        for sub in subtags {
            let len = sub.len();
            if !(2..=8).contains(&len) || !sub.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(IdValidationError::invalid(
                    "LocaleCode",
                    "subtag must be 2-8 alphanumeric ASCII characters",
                ));
            }
        }
        Ok(Self(value))
    }

    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(value: impl Into<String>) -> Self {
        // SAFETY: `new` is the infallible constructor reserved for inputs the caller
        // has already validated (compile-time literals, values that
        // round-tripped through `try_new` at a boundary). Untrusted input must
        // go through `try_new`.
        Self::try_new(value).expect("LocaleCode validation failed")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LocaleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for LocaleCode {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for LocaleCode {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for LocaleCode {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for LocaleCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LocaleCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for LocaleCode {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &LocaleCode {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
