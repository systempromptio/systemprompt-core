//! The `aud` claim domain: which surface a token is minted for.
//!
//! `ui` is the audience of a person's UI session, the bearer token a
//! `person_ui` client holds for the person who signed in through it
//! ([`JwtClaims::ui_session_well_formed`] is its shape; the profile's
//! `security.ui_session` block sets its lifetimes). It is outside
//! [`JwtAudience::FIRST_PARTY`] and [`JwtAudience::service`], so every surface
//! that validates against those refuses it; only a surface that names
//! [`JwtAudience::FIRST_PARTY_AND_UI`] admits it.
//!
//! [`JwtClaims::ui_session_well_formed`]: crate::auth::JwtClaims::ui_session_well_formed
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::errors::ParseEnumError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum JwtAudience {
    Web,
    Api,
    A2a,
    Mcp,
    Internal,
    Bridge,
    Hook,
    Ui,
    #[serde(untagged)]
    Resource(String),
}

impl JwtAudience {
    pub const FIRST_PARTY: &'static [Self] = &[Self::Web, Self::Api, Self::A2a, Self::Mcp];

    pub const FIRST_PARTY_AND_UI: &'static [Self] =
        &[Self::Web, Self::Api, Self::A2a, Self::Mcp, Self::Ui];

    pub const fn as_str(&self) -> &str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Internal => "internal",
            Self::Bridge => "bridge",
            Self::Hook => "hook",
            Self::Ui => "ui",
            Self::Resource(s) => s.as_str(),
        }
    }

    pub fn standard() -> Vec<Self> {
        Self::FIRST_PARTY.to_vec()
    }

    pub fn service() -> Vec<Self> {
        vec![Self::Api, Self::Mcp, Self::A2a, Self::Internal]
    }
}

impl fmt::Display for JwtAudience {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for JwtAudience {
    type Err = ParseEnumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "web" => Ok(Self::Web),
            "api" => Ok(Self::Api),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "internal" => Ok(Self::Internal),
            "bridge" => Ok(Self::Bridge),
            "hook" => Ok(Self::Hook),
            "ui" => Ok(Self::Ui),
            _ => Ok(Self::Resource(s.to_owned())),
        }
    }
}
