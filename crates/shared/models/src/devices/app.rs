//! The reverse-DNS name an app is published under.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

const MAX_APP_IDENTIFIER_LEN: usize = 255;

/// An Apple bundle identifier or an Android package name, e.g.
/// `io.example.app`: at least two `.`-separated segments of ASCII letters,
/// digits, `_` and `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct AppIdentifier(String);

/// A string that is not a valid [`AppIdentifier`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid app identifier {value:?}: {reason}")]
pub struct AppIdentifierError {
    pub value: String,
    pub reason: &'static str,
}

impl AppIdentifier {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AppIdentifierError> {
        let value = value.into();
        match violation(&value) {
            Some(reason) => Err(AppIdentifierError { value, reason }),
            None => Ok(Self(value)),
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn violation(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return Some("must not be empty");
    }
    if value.len() > MAX_APP_IDENTIFIER_LEN {
        return Some("must be at most 255 bytes");
    }
    if value.split('.').count() < 2 {
        return Some("must have at least two '.'-separated segments");
    }
    if value.split('.').any(str::is_empty) {
        return Some("segments must not be empty");
    }
    if !value
        .bytes()
        .all(|b| b == b'.' || b == b'_' || b == b'-' || b.is_ascii_alphanumeric())
    {
        return Some("must contain only ASCII letters, digits, '_', '-' and '.'");
    }
    None
}

impl fmt::Display for AppIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AppIdentifier {
    type Err = AppIdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

crate::notifications::string_form::string_serde!(AppIdentifier, "AppIdentifier");
