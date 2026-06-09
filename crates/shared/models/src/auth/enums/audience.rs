//! The `aud` claim domain: which surface a token is minted for.

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
    #[serde(untagged)]
    Resource(String),
}

impl JwtAudience {
    pub const FIRST_PARTY: &'static [Self] = &[Self::Web, Self::Api, Self::A2a, Self::Mcp];

    pub const fn as_str(&self) -> &str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Internal => "internal",
            Self::Bridge => "bridge",
            Self::Hook => "hook",
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
            _ => Ok(Self::Resource(s.to_owned())),
        }
    }
}
