//! [`McpToolExecutionStatus`] — the `mcp_tool_executions.status` vocabulary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolExecutionStatus {
    #[default]
    Pending,
    Success,
    Failed,
    Timeout,
}

impl McpToolExecutionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
        }
    }

    #[must_use]
    pub const fn from_error(has_error: bool) -> Self {
        if has_error {
            Self::Failed
        } else {
            Self::Success
        }
    }
}

impl fmt::Display for McpToolExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown MCP tool execution status `{0}`")]
pub struct UnknownMcpToolExecutionStatus(pub String);

impl FromStr for McpToolExecutionStatus {
    type Err = UnknownMcpToolExecutionStatus;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "timeout" => Ok(Self::Timeout),
            other => Err(UnknownMcpToolExecutionStatus(other.to_owned())),
        }
    }
}
