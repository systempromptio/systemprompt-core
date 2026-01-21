//! CLI Gateway models for remote command execution.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliExecuteRequest {
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

const fn default_timeout() -> u64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliOutputEvent {
    Started { pid: u32 },
    Stdout { data: String },
    Stderr { data: String },
    ExitCode { code: i32 },
    Error { message: String },
}
