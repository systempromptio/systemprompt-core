//! [`SecretPattern`]: a custom-secret name or a prefix of names.
//!
//! A member declares what it reads with one (`Contributions::secret`,
//! `Contributions::secret_prefix`) and the operator grants it in the profile
//! (`extensions.secrets`).
//!
//! The text form is an exact name (`acme_api_key`) or a prefix ending in one
//! `*` (`acme_*`); no other wildcard is accepted, and neither an empty name,
//! a bare `*` nor an empty prefix is. Matching is byte-exact and so
//! case-sensitive: `acme_*` covers `acme_x`, never `acmex` or `ACME_x`, and
//! a prefix covers only a name longer than itself.
//!
//! [`CORE_SECRET_NAMES`] is the one list of the fixed custom-secret names a
//! core crate reads by a constant rather than through a config field: the
//! platform's pinned keys, the licence signing secret and the MCP credential
//! broker secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_identifiers::SecretName;

use crate::secret::PINNED_SECRET_KEYS;

pub const LICENCE_SIGNING_SECRET: &str = "licence_signing_secret";

pub const MCP_CREDENTIAL_BROKER_SECRET: &str = "mcp_credential_broker_secret";

pub const CORE_SECRET_NAMES: &[&str] = &[LICENCE_SIGNING_SECRET, MCP_CREDENTIAL_BROKER_SECRET];

#[must_use]
pub fn is_core_secret_name(name: &str) -> bool {
    PINNED_SECRET_KEYS
        .iter()
        .chain(CORE_SECRET_NAMES)
        .any(|core| core.eq_ignore_ascii_case(name))
}

const WILDCARD: char = '*';

/// One exact custom-secret name, or a prefix of names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecretPattern {
    Exact(SecretName),
    Prefix(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretPatternError {
    #[error("a secret name or prefix is empty")]
    Empty,
    #[error("a bare `*` grants every secret; name the secret or a prefix such as `acme_*`")]
    BareWildcard,
    #[error("secret pattern '{pattern}' places `*` other than once at its end")]
    MisplacedWildcard { pattern: String },
}

impl SecretPattern {
    pub fn parse(text: &str) -> Result<Self, SecretPatternError> {
        match text.strip_suffix(WILDCARD) {
            _ if text.is_empty() => Err(SecretPatternError::Empty),
            Some("") => Err(SecretPatternError::BareWildcard),
            Some(prefix) if prefix.contains(WILDCARD) => {
                Err(SecretPatternError::MisplacedWildcard {
                    pattern: text.to_owned(),
                })
            },
            Some(prefix) => Ok(Self::Prefix(prefix.to_owned())),
            None if text.contains(WILDCARD) => Err(SecretPatternError::MisplacedWildcard {
                pattern: text.to_owned(),
            }),
            None => Ok(Self::Exact(SecretName::new(text))),
        }
    }

    pub fn prefix(prefix: &str) -> Result<Self, SecretPatternError> {
        Self::parse(&format!("{prefix}{WILDCARD}"))
    }

    #[must_use]
    pub fn covers(&self, name: &str) -> bool {
        match self {
            Self::Exact(exact) => exact.as_str() == name,
            Self::Prefix(prefix) => name.len() > prefix.len() && name.starts_with(prefix.as_str()),
        }
    }

    #[must_use]
    pub fn covers_pattern(&self, other: &Self) -> bool {
        match (self, other) {
            (_, Self::Exact(name)) => self.covers(name.as_str()),
            (Self::Exact(_), Self::Prefix(_)) => false,
            (Self::Prefix(outer), Self::Prefix(inner)) => inner.starts_with(outer.as_str()),
        }
    }
}

impl fmt::Display for SecretPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(name) => f.write_str(name.as_str()),
            Self::Prefix(prefix) => write!(f, "{prefix}{WILDCARD}"),
        }
    }
}

impl Serialize for SecretPattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SecretPattern {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for SecretPattern {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("SecretPattern")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[^*]+\\*?$",
        })
    }
}
