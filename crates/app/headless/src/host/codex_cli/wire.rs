//! The `codex exec --json` wire shapes, after Codex CLI 0.157.0's
//! `codex-rs/exec/src/exec_events.rs`. Only the fields the decoder reads are
//! declared; unknown events and items fall to `Other`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(super) enum ThreadEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.completed")]
    TurnCompleted {
        #[serde(default)]
        usage: Usage,
    },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: Message },
    #[serde(rename = "item.started")]
    ItemStarted { item: Item },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: Item },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct Usage {
    #[serde(default)]
    pub(super) input_tokens: i64,
    #[serde(default)]
    pub(super) output_tokens: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct Message {
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Item {
    pub(super) id: String,
    #[serde(flatten)]
    pub(super) details: ItemDetails,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ItemDetails {
    AgentMessage {
        text: String,
    },
    CommandExecution {
        command: String,
        #[serde(default)]
        aggregated_output: String,
        #[serde(default)]
        exit_code: Option<i32>,
        #[serde(default)]
        status: String,
    },
    FileChange {
        #[serde(default)]
        changes: Vec<FileChange>,
        #[serde(default)]
        status: String,
    },
    McpToolCall {
        server: String,
        tool: String,
        // JSON: the MCP tool's own input schema, relayed verbatim.
        #[serde(default)]
        arguments: serde_json::Value,
        // JSON: an MCP `CallToolResult`, relayed as text.
        #[serde(default)]
        result: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<Message>,
    },
    WebSearch {
        #[serde(default)]
        query: String,
    },
    Error {
        message: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct FileChange {
    pub(super) path: String,
    pub(super) kind: String,
}
