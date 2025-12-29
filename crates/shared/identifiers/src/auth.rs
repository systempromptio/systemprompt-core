//! Authentication identifier types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// JWT token (always required after `SessionMiddleware`)
/// Can be user JWT or anonymous JWT
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct JwtToken(String);

impl JwtToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JwtToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for JwtToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for JwtToken {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for JwtToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
