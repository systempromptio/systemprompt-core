//! The verdicts the instance answers tool calls with.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ScriptedInstanceError;
use crate::file;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictScript {
    #[serde(default)]
    pub default: Verdict,
    #[serde(default)]
    pub tools: BTreeMap<String, Verdict>,
}

impl VerdictScript {
    pub fn allow_all() -> Self {
        Self::default()
    }

    pub fn for_tool(mut self, tool: impl Into<String>, verdict: Verdict) -> Self {
        self.tools.insert(tool.into(), verdict);
        self
    }

    pub fn verdict_for(&self, tool: &str) -> &Verdict {
        self.tools.get(tool).unwrap_or(&self.default)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ScriptedInstanceError> {
        file::read_json(path.as_ref())
    }

    pub fn from_json(json: &str) -> Result<Self, ScriptedInstanceError> {
        file::parse_json(json)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    #[default]
    Allow,
    Deny {
        reason: String,
    },
    Delay {
        millis: u64,
    },
    Error {
        status: u16,
    },
}
