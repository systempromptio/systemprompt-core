//! Call source classification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::errors::ParseEnumError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallSource {
    Agentic,
    Direct,
    Ephemeral,
}

impl FromStr for CallSource {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "agentic" => Ok(Self::Agentic),
            "direct" => Ok(Self::Direct),
            "ephemeral" => Ok(Self::Ephemeral),
            _ => Err(ParseEnumError::new("call_source", s)),
        }
    }
}

impl CallSource {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Direct => "direct",
            Self::Ephemeral => "ephemeral",
        }
    }
}
