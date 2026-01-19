//! CLI Gateway models for remote command execution.

use axum::response::sse::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliExecuteRequest {
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
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

impl CliOutputEvent {
    pub fn to_sse_event(&self) -> Event {
        Event::default()
            .event("cli")
            .json_data(self)
            .unwrap_or_else(|_| Event::default().event("cli").data("{}"))
    }
}
