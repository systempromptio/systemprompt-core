//! Decodes `opencode run --format json` lines into [`RunEventPayload`]s.
//!
//! The first `sessionID` becomes `Init`; a line naming a different session
//! is refused with a `Runner` error. `text` becomes `AssistantText`; a
//! completed or errored `tool_use` becomes `ToolUse` + `ToolResult`, once per
//! part id; `step_finish` becomes a `Result` carrying that step's tokens; an
//! `error` event becomes a `Runner` error prefixed with its type. A line that
//! is not JSON is a `Runner` warning. Decoding stops (one `Runner` error)
//! once the stream passes [`MAX_STREAM_BYTES`] or [`MAX_EVENTS`]; a line over
//! [`MAX_LINE_BYTES`] is dropped with a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::HostSessionId;

use crate::event::{RunEventPayload, RunnerLevel};
use crate::host::HostEvent;

use super::wire::{StepFinishPart, StreamEvent, ToolPart};

pub const MAX_STREAM_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVENTS: usize = 8192;
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const STEP_FINISH: &str = "step_finish";
pub const ERROR_PREFIX: &str = "opencode error";
pub const DECODER_PREFIX: &str = "opencode ";

/// Decode state carried across the lines of one run.
#[derive(Debug, Default)]
pub struct DecodeState {
    pub model: String,
    pub cwd: Option<String>,
    session: Option<String>,
    seen_tools: BTreeSet<String>,
    bytes: usize,
    events: usize,
    exhausted: bool,
}

pub fn decode_line(state: &mut DecodeState, line: &str) -> Vec<HostEvent> {
    if state.exhausted {
        return Vec::new();
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    state.bytes = state.bytes.saturating_add(line.len());
    state.events += 1;
    if state.bytes > MAX_STREAM_BYTES || state.events > MAX_EVENTS {
        state.exhausted = true;
        return vec![payloads(vec![runner(
            RunnerLevel::Error,
            format!(
                "opencode stream exceeded {MAX_EVENTS} events or {MAX_STREAM_BYTES} bytes; \
                 decoding stopped"
            ),
        )])];
    }
    if trimmed.len() > MAX_LINE_BYTES {
        return vec![payloads(vec![runner(
            RunnerLevel::Warn,
            format!("opencode line of {} bytes dropped", trimmed.len()),
        )])];
    }
    let event = match serde_json::from_str::<StreamEvent>(trimmed) {
        Ok(event) => event,
        Err(error) => {
            return vec![payloads(vec![runner(
                RunnerLevel::Warn,
                format!("undecodable opencode line ({error}): {trimmed}"),
            )])];
        },
    };
    let (session, body) = match event {
        StreamEvent::StepStart { session } => (session, Vec::new()),
        StreamEvent::Text { session, part } => (
            session,
            vec![RunEventPayload::AssistantText { text: part.text }],
        ),
        StreamEvent::ToolUse { session, part } => (session, tool(state, part)),
        StreamEvent::StepFinish { session, part } => (session, vec![step(&part)]),
        StreamEvent::Error { session, error } => {
            let kind = error.kind.unwrap_or_else(|| "unknown".to_owned());
            let message = error.message.unwrap_or_default();
            (
                session,
                vec![runner(
                    RunnerLevel::Error,
                    format!("{ERROR_PREFIX} {kind}: {message}"),
                )],
            )
        },
        StreamEvent::Other => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Some(session) = session {
        match session_check(state, &session) {
            SessionCheck::Same => {},
            SessionCheck::First(event) => out.push(event),
            SessionCheck::Mixed(event) => return vec![event],
        }
    }
    if !body.is_empty() {
        out.push(payloads(body));
    }
    out
}

enum SessionCheck {
    Same,
    First(HostEvent),
    Mixed(HostEvent),
}

fn session_check(state: &mut DecodeState, session: &str) -> SessionCheck {
    match &state.session {
        Some(known) if known == session => SessionCheck::Same,
        Some(known) => SessionCheck::Mixed(payloads(vec![runner(
            RunnerLevel::Error,
            format!("opencode switched session from {known} to {session}; line refused"),
        )])),
        None => {
            state.session = Some(session.to_owned());
            let Ok(session_id) = HostSessionId::try_new(session) else {
                return SessionCheck::First(payloads(vec![runner(
                    RunnerLevel::Warn,
                    format!("opencode session id rejected: {session}"),
                )]));
            };
            SessionCheck::First(HostEvent {
                payloads: vec![RunEventPayload::Init {
                    session_id: session_id.clone(),
                    model: state.model.clone(),
                    tools: Vec::new(),
                    cwd: state.cwd.clone(),
                }],
                session_id: Some(session_id),
            })
        },
    }
}

fn tool(state: &mut DecodeState, part: ToolPart) -> Vec<RunEventPayload> {
    let finished = matches!(part.state.status.as_str(), "completed" | "error");
    let Some(id) = part.id.or(part.part_id) else {
        return Vec::new();
    };
    if !finished || !state.seen_tools.insert(id.clone()) {
        return Vec::new();
    }
    let is_error = part.state.status == "error";
    let content = if is_error {
        part.state.error.unwrap_or_default()
    } else {
        part.state.output.unwrap_or_default()
    };
    vec![
        RunEventPayload::ToolUse {
            tool_use_id: id.clone(),
            name: part.tool,
            input: part.state.input,
        },
        RunEventPayload::ToolResult {
            tool_use_id: id,
            content,
            is_error,
        },
    ]
}

fn step(part: &StepFinishPart) -> RunEventPayload {
    RunEventPayload::Result {
        subtype: STEP_FINISH.to_owned(),
        is_error: false,
        num_turns: 1,
        duration_ms: 0,
        input_tokens: part.tokens.input,
        output_tokens: part.tokens.output,
        cost_microdollars: 0,
        structured_output: None,
        error: None,
    }
}

const fn runner(level: RunnerLevel, message: String) -> RunEventPayload {
    RunEventPayload::Runner { message, level }
}

const fn payloads(payloads: Vec<RunEventPayload>) -> HostEvent {
    HostEvent {
        payloads,
        session_id: None,
    }
}
