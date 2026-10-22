//! [`QuotaSubjectKind`] — what a quota bucket is keyed by.
//!
//! The two principals are closed (`user`, `api_key`); every other kind is a
//! scope dimension named by the gateway policy's `subject`, so the vocabulary
//! is open and the database constrains only that a kind is present. A
//! dimension name obeys [`ScopeDimension`]'s validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ScopeDimension;
use crate::error::IdValidationError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum QuotaSubjectKind {
    #[default]
    User,
    ApiKey,
    Dimension(ScopeDimension),
}

impl QuotaSubjectKind {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::User => "user",
            Self::ApiKey => "api_key",
            Self::Dimension(name) => name.as_str(),
        }
    }

    #[must_use]
    pub const fn is_principal(&self) -> bool {
        matches!(self, Self::User | Self::ApiKey)
    }
}

impl fmt::Display for QuotaSubjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("quota subject kind is neither a principal nor a scope dimension: {0}")]
pub struct InvalidQuotaSubjectKind(#[from] IdValidationError);

impl FromStr for QuotaSubjectKind {
    type Err = InvalidQuotaSubjectKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "api_key" => Ok(Self::ApiKey),
            other => Ok(Self::Dimension(ScopeDimension::try_new(other)?)),
        }
    }
}

impl Serialize for QuotaSubjectKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for QuotaSubjectKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(D::Error::custom)
    }
}
