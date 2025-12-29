//! Call source classification.

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallSource {
    Agentic,
    Direct,
    Ephemeral,
}

impl FromStr for CallSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "agentic" => Ok(Self::Agentic),
            "direct" => Ok(Self::Direct),
            "ephemeral" => Ok(Self::Ephemeral),
            _ => Err(anyhow!("Invalid CallSource: {s}")),
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
