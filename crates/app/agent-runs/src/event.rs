//! The host-generic run event contract.
//!
//! Every host adapter decodes its own stream (Claude Code `stream-json`, Codex
//! CLI `--json`) into [`RunEventPayload`]s; the executor numbers them into
//! [`RunEvent`]s and persists them in `agent_run_events`, whose `kind`
//! column checks the eight [`RunEventKind`] spellings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::HostSessionId;

use crate::macros::text_enum;

text_enum!(
    RunEventKind, UnknownEventKind {
        Init => "init",
        AssistantText => "assistant_text",
        ToolUse => "tool_use",
        ToolResult => "tool_result",
        Hook => "hook",
        Result => "result",
        Stderr => "stderr",
        Runner => "runner",
    }
);

/// A governance hook's verdict on a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Allow,
    Deny,
    Pending,
}

/// Severity of an executor-authored [`RunEventPayload::Runner`] line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerLevel {
    Info,
    Warn,
    Error,
}

// JSON: protocol boundary — a tool call's `input` and a structured final
// answer are whatever the tool's or the caller's schema says, relayed verbatim
// from the host's stream into the `payload` JSONB column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunEventPayload {
    Init {
        session_id: HostSessionId,
        model: String,
        #[serde(default)]
        tools: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    AssistantText {
        text: String,
    },
    ToolUse {
        tool_use_id: String,
        name: String,
        // JSON: the tool's own input schema, relayed verbatim.
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Hook {
        name: String,
        decision: HookDecision,
        #[serde(default)]
        policy: Option<String>,
    },
    Result {
        subtype: String,
        is_error: bool,
        num_turns: i64,
        duration_ms: i64,
        input_tokens: i64,
        output_tokens: i64,
        cost_microdollars: i64,
        // JSON: shaped by the caller's `RunRequest::output_schema`.
        #[serde(default)]
        structured_output: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<String>,
    },
    Stderr {
        line: String,
    },
    Runner {
        message: String,
        level: RunnerLevel,
    },
}

impl RunEventPayload {
    #[must_use]
    pub const fn kind(&self) -> RunEventKind {
        match self {
            Self::Init { .. } => RunEventKind::Init,
            Self::AssistantText { .. } => RunEventKind::AssistantText,
            Self::ToolUse { .. } => RunEventKind::ToolUse,
            Self::ToolResult { .. } => RunEventKind::ToolResult,
            Self::Hook { .. } => RunEventKind::Hook,
            Self::Result { .. } => RunEventKind::Result,
            Self::Stderr { .. } => RunEventKind::Stderr,
            Self::Runner { .. } => RunEventKind::Runner,
        }
    }

    #[must_use]
    pub fn tool_use_id(&self) -> Option<&str> {
        match self {
            Self::ToolUse { tool_use_id, .. } | Self::ToolResult { tool_use_id, .. } => {
                Some(tool_use_id)
            },
            _ => None,
        }
    }
}

/// One numbered, persisted event of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    pub seq: i64,
    pub kind: RunEventKind,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub payload: RunEventPayload,
    pub created_at: DateTime<Utc>,
}
