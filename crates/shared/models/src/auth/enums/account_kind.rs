//! Whether a user row is a person or a non-human service account.
//!
//! A service account is the identity a workflow or agent farm acts as when
//! it writes to an external system. It may hold API keys and in-process JWTs
//! but never an interactive credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::errors::ParseEnumError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    #[default]
    Person,
    Service,
}

impl AccountKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Service => "service",
        }
    }

    pub const fn is_service(&self) -> bool {
        matches!(self, Self::Service)
    }

    pub const fn allows_interactive_login(&self) -> bool {
        matches!(self, Self::Person)
    }
}

impl fmt::Display for AccountKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AccountKind {
    type Err = ParseEnumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "person" => Ok(Self::Person),
            "service" => Ok(Self::Service),
            _ => Err(ParseEnumError::new("account_kind", s)),
        }
    }
}
