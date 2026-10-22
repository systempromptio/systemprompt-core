//! [`EnforcementMode`] — the one vocabulary for "halt on a finding" versus
//! "record it and let the call through", shared by every enforcement plane
//! (governance chain, safety scanners, quota windows).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnforcementMode {
    #[default]
    Enforce,
    Warn,
}

impl EnforcementMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Warn => "warn",
        }
    }

    #[must_use]
    pub const fn is_warn(self) -> bool {
        matches!(self, Self::Warn)
    }
}

impl fmt::Display for EnforcementMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown enforcement mode `{0}`; expected `enforce` or `warn`")]
pub struct UnknownEnforcementMode(pub String);

impl FromStr for EnforcementMode {
    type Err = UnknownEnforcementMode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enforce" => Ok(Self::Enforce),
            "warn" => Ok(Self::Warn),
            other => Err(UnknownEnforcementMode(other.to_owned())),
        }
    }
}
