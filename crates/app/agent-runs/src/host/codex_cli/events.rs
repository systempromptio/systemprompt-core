//! Decodes `codex exec --json` lines into [`RunEventPayload`]s.
//!
//! `thread.started` becomes `Init`; an `agent_message` item `AssistantText`;
//! a `command_execution`, `mcp_tool_call`, `file_change` or `web_search`
//! item a `ToolUse` when it starts and a `ToolResult` when it completes (a
//! completed item never seen starting yields both). `turn.completed` and
//! `turn.failed` become `Result`. A top-level `error` and an `error` item are
//! retry notices that precede `turn.failed`, so they are `Runner` warnings.
//! Unknown events and items decode to nothing; a line that is not JSON at all
//! becomes a `Runner` warning carrying the line.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::HostSessionId;

use crate::event::{RunEventPayload, RunnerLevel};
use crate::host::HostEvent;

use super::wire::{Item, ItemDetails, ThreadEvent, Usage};

pub const TURN_COMPLETED: &str = "turn_completed";
pub const TURN_FAILED: &str = "turn_failed";

/// Decode state carried across the lines of one run.
#[derive(Debug, Default)]
pub struct DecodeState {
    pub model: String,
    pub cwd: Option<String>,
    started: BTreeSet<String>,
}

struct Tool {
    name: String,
    // JSON: the tool input relayed into `ToolUse::input`.
    input: serde_json::Value,
    content: String,
    is_error: bool,
}

pub fn decode_line(state: &mut DecodeState, line: &str) -> Vec<HostEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let event = match serde_json::from_str::<ThreadEvent>(trimmed) {
        Ok(event) => event,
        Err(error) => {
            return vec![payloads(vec![warn(format!(
                "undecodable codex line ({error}): {trimmed}"
            ))])];
        },
    };
    match event {
        ThreadEvent::ThreadStarted { thread_id } => init(state, &thread_id),
        ThreadEvent::TurnCompleted { usage } => vec![payloads(vec![result(&usage, None)])],
        ThreadEvent::TurnFailed { error } => {
            vec![payloads(vec![result(
                &Usage::default(),
                Some(error.message),
            )])]
        },
        ThreadEvent::ItemStarted { item } => started(state, &item),
        ThreadEvent::ItemCompleted { item } => completed(state, item),
        ThreadEvent::Error { message } => vec![payloads(vec![warn(message)])],
        ThreadEvent::Other => Vec::new(),
    }
}

fn init(state: &DecodeState, thread_id: &str) -> Vec<HostEvent> {
    let Ok(session_id) = HostSessionId::try_new(thread_id) else {
        return vec![payloads(vec![warn(format!(
            "codex thread id rejected: {thread_id}"
        ))])];
    };
    vec![HostEvent {
        payloads: vec![RunEventPayload::Init {
            session_id: session_id.clone(),
            model: state.model.clone(),
            tools: Vec::new(),
            cwd: state.cwd.clone(),
        }],
        session_id: Some(session_id),
    }]
}

fn started(state: &mut DecodeState, item: &Item) -> Vec<HostEvent> {
    let Some(tool) = tool(&item.details) else {
        return Vec::new();
    };
    state.started.insert(item.id.clone());
    vec![payloads(vec![tool_use(&item.id, &tool)])]
}

fn completed(state: &mut DecodeState, item: Item) -> Vec<HostEvent> {
    match &item.details {
        ItemDetails::AgentMessage { text } => {
            return vec![payloads(vec![RunEventPayload::AssistantText {
                text: text.clone(),
            }])];
        },
        ItemDetails::Error { message } => return vec![payloads(vec![warn(message.clone())])],
        _ => {},
    }
    let Some(tool) = tool(&item.details) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if !state.started.remove(&item.id) {
        out.push(tool_use(&item.id, &tool));
    }
    out.push(RunEventPayload::ToolResult {
        tool_use_id: item.id,
        content: tool.content,
        is_error: tool.is_error,
    });
    vec![payloads(out)]
}

fn tool(details: &ItemDetails) -> Option<Tool> {
    let tool = match details {
        ItemDetails::CommandExecution {
            command,
            aggregated_output,
            exit_code,
            status,
        } => Tool {
            name: "command_execution".to_owned(),
            input: serde_json::json!({ "command": command }),
            content: aggregated_output.clone(),
            is_error: status == "failed"
                || status == "declined"
                || exit_code.is_some_and(|c| c != 0),
        },
        ItemDetails::FileChange { changes, status } => Tool {
            name: "file_change".to_owned(),
            input: serde_json::json!({ "changes": changes }),
            content: changes
                .iter()
                .map(|c| format!("{} {}", c.kind, c.path))
                .collect::<Vec<_>>()
                .join("\n"),
            is_error: status == "failed",
        },
        ItemDetails::McpToolCall {
            server,
            tool,
            arguments,
            result,
            error,
        } => Tool {
            name: format!("mcp__{server}__{tool}"),
            input: arguments.clone(),
            content: error.as_ref().map_or_else(
                || {
                    result
                        .as_ref()
                        .map_or_else(String::new, ToString::to_string)
                },
                |e| e.message.clone(),
            ),
            is_error: error.is_some(),
        },
        ItemDetails::WebSearch { query } => Tool {
            name: "web_search".to_owned(),
            input: serde_json::json!({ "query": query }),
            content: String::new(),
            is_error: false,
        },
        ItemDetails::AgentMessage { .. } | ItemDetails::Error { .. } | ItemDetails::Other => {
            return None;
        },
    };
    Some(tool)
}

fn tool_use(id: &str, tool: &Tool) -> RunEventPayload {
    RunEventPayload::ToolUse {
        tool_use_id: id.to_owned(),
        name: tool.name.clone(),
        input: tool.input.clone(),
    }
}

fn result(usage: &Usage, error: Option<String>) -> RunEventPayload {
    RunEventPayload::Result {
        subtype: if error.is_some() {
            TURN_FAILED
        } else {
            TURN_COMPLETED
        }
        .to_owned(),
        is_error: error.is_some(),
        num_turns: 1,
        duration_ms: 0,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost_microdollars: 0,
        structured_output: None,
        error,
    }
}

const fn warn(message: String) -> RunEventPayload {
    RunEventPayload::Runner {
        message,
        level: RunnerLevel::Warn,
    }
}

const fn payloads(payloads: Vec<RunEventPayload>) -> HostEvent {
    HostEvent {
        payloads,
        session_id: None,
    }
}
