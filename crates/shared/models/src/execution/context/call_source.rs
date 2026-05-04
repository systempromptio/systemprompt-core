//! Call source classification.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::errors::ParseEnumError;

/// Classification of where a request originated, used for routing,
/// rate-limiting, and audit log filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallSource {
    /// Issued by an autonomous agent loop.
    Agentic,
    /// Issued directly by a human user (UI, CLI).
    Direct,
    /// One-off ephemeral / test invocation.
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
