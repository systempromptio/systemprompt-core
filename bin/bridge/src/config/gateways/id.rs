//! The gateway profile identifier: a directory name and a credential-store
//! account suffix, so it is restricted to lowercase ASCII, digits and `-`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ValidatedUrl;

const MAX_LEN: usize = 48;
const DEFAULT: &str = "default";

#[derive(Debug, thiserror::Error)]
pub enum ProfileIdError {
    #[error("gateway profile id is empty")]
    Empty,
    #[error("gateway profile id is longer than {MAX_LEN} characters")]
    TooLong,
    #[error("gateway profile id `{0}` may hold only a-z, 0-9 and `-`")]
    Charset(String),
    #[error("gateway URL is invalid: {0}")]
    Url(#[source] systemprompt_identifiers::error::IdValidationError),
}

/// Names one directory under `gateways/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GatewayProfileId(String);

impl GatewayProfileId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProfileIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProfileIdError::Empty);
        }
        if value.len() > MAX_LEN {
            return Err(ProfileIdError::TooLong);
        }
        if !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || value.starts_with('-')
        {
            return Err(ProfileIdError::Charset(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn default_profile() -> Self {
        Self(DEFAULT.to_owned())
    }

    #[must_use]
    pub fn from_url(url: &ValidatedUrl) -> Self {
        let host = url::Url::parse(url.as_str())
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        let mapped: String = host
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let mut slug = mapped.trim_matches('-').to_owned();
        slug.truncate(MAX_LEN - 4);
        Self::try_new(slug).unwrap_or_else(|_| Self("gateway".to_owned()))
    }

    #[must_use]
    pub fn with_suffix(&self, n: u32) -> Self {
        Self(format!("{}-{n}", self.0))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GatewayProfileId {
    type Error = ProfileIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<GatewayProfileId> for String {
    fn from(id: GatewayProfileId) -> Self {
        id.0
    }
}

impl std::fmt::Display for GatewayProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
