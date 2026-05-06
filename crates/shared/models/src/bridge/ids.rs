//! Typed identifiers for bridge manifest wire fields.
//!
//! Each newtype is `#[serde(transparent)]` so it serialises to and
//! from a plain JSON string — the typing is purely a Rust-side
//! invariant. `non_empty` IDs reject the empty string at deserialise
//! time; [`Sha256Digest`] additionally enforces 64 lowercase hex
//! characters; [`ManifestSignature`] is a passthrough wrapper for the
//! base64-encoded detached ed25519 signature carried alongside every
//! manifest.
//!
//! These IDs are defined here (rather than in `systemprompt-identifiers`)
//! because they are bridge-protocol-scoped: they appear only inside
//! `/v1/bridge/*` payloads. They share the same shape as the broader
//! identifier crate but a parallel definition keeps the bridge wire
//! contract self-contained.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum IdValidationError {
    #[error("{type_name} cannot be empty")]
    Empty { type_name: &'static str },
    #[error("{type_name} is invalid: {reason}")]
    Invalid {
        type_name: &'static str,
        reason: String,
    },
}

impl IdValidationError {
    #[must_use]
    pub const fn empty(type_name: &'static str) -> Self {
        Self::Empty { type_name }
    }

    pub fn invalid(type_name: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            type_name,
            reason: reason.into(),
        }
    }
}

macro_rules! shared_non_empty_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(IdValidationError::empty(stringify!($name)));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdValidationError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdValidationError;
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = IdValidationError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_new(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::try_new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}

shared_non_empty_id!(PluginId);
shared_non_empty_id!(SkillId);
shared_non_empty_id!(SkillName);
shared_non_empty_id!(ManagedMcpServerName);
shared_non_empty_id!(ToolName);

/// Detached ed25519 signature of the canonicalised manifest body.
///
/// Wire format is base64 standard with padding; the type itself is a
/// passthrough wrapper (no validation) — invalid base64 is rejected
/// at verification time, not at parse time, so unsigned manifests
/// can still round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManifestSignature(String);

impl ManifestSignature {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for ManifestSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ManifestSignature {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for ManifestSignature {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ManifestSignature {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Lowercase hex SHA-256 digest. Validated as exactly 64 hex chars
/// `[0-9a-f]` so manifest comparisons are normalised — any
/// upper-case or shorter input is rejected at deserialise time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.len() != 64 {
            return Err(IdValidationError::invalid(
                "Sha256Digest",
                format!("expected 64 hex chars, got {}", value.len()),
            ));
        }
        if !value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(IdValidationError::invalid(
                "Sha256Digest",
                "expected lowercase hex characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Sha256Digest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Sha256Digest> for String {
    fn from(id: Sha256Digest) -> Self {
        id.0
    }
}

impl TryFrom<String> for Sha256Digest {
    type Error = IdValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for Sha256Digest {
    type Error = IdValidationError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for Sha256Digest {
    type Err = IdValidationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolPolicy {
    Allow,
    Deny,
    Prompt,
}

impl fmt::Display for ToolPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => f.write_str("allow"),
            Self::Deny => f.write_str("deny"),
            Self::Prompt => f.write_str("prompt"),
        }
    }
}
