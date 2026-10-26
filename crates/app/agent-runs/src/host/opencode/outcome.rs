//! Classifies a finished `opencode run` into a [`RunOutcome`].
//!
//! `OpenCode` emits no terminal result event and its exit code is unreliable
//! (an invalid tool call exits 0), so the outcome comes from events alone.
//! Precedence: cancelled, timed out, an `error` event, then a stream with no
//! `text` and no `step_finish`, then the structured output check. A
//! An `error` event is classed by its `type` alone, never by its message: a
//! provider route, auth, transport or upstream failure is the connector's,
//! a quota is the budget's, an unusable model output is the output's, and
//! anything else is the host's. Tokens and turns are summed over every
//! `step_finish`; cost is the gateway's to attribute and stays 0 here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::HostSessionId;

use super::events::{DECODER_PREFIX, ERROR_PREFIX};
use crate::event::{RunEventPayload, RunnerLevel};
use crate::host::ExitInfo;
use crate::outcome::{ErrorClass, RunOutcome, RunStatus};
use crate::request::RunRequest;

const CONNECTOR_KINDS: [&str; 8] = [
    "provider.no-route",
    "provider.auth",
    "provider.transport",
    "provider.timeout",
    "provider.rate-limit",
    "provider.internal",
    "provider.error",
    "provider.unknown",
];
const BUDGET_KINDS: [&str; 2] = ["provider.quota", "provider.free-limit"];
const OUTPUT_KINDS: [&str; 1] = ["provider.invalid-output"];

#[must_use]
pub fn classify(exit: ExitInfo, events: &[RunEventPayload], request: &RunRequest) -> RunOutcome {
    let mut outcome = RunOutcome {
        status: RunStatus::Succeeded,
        error_class: ErrorClass::Ok,
        session_id: session_id(events),
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 0,
        duration_ms: exit.duration_ms,
        structured_output: None,
        exit_code: exit.code,
        error: None,
        final_text: RunOutcome::last_assistant_text(events),
    };
    for event in events {
        if let RunEventPayload::Result {
            input_tokens,
            output_tokens,
            ..
        } = event
        {
            outcome.input_tokens += input_tokens;
            outcome.output_tokens += output_tokens;
            outcome.turns += 1;
        }
    }
    let produced = outcome.turns > 0 || outcome.final_text.is_some();
    let (status, class, error) = if exit.cancelled {
        (
            RunStatus::Cancelled,
            ErrorClass::Ok,
            Some("run cancelled".to_owned()),
        )
    } else if exit.timed_out {
        (
            RunStatus::TimedOut,
            ErrorClass::Timeout,
            Some("wall clock exceeded".to_owned()),
        )
    } else if let Some(message) = host_error(events) {
        (RunStatus::Failed, failure_class(&message), Some(message))
    } else if !produced {
        (
            RunStatus::Failed,
            ErrorClass::HostCrashed,
            Some(format!(
                "opencode produced no text and no step (exit {:?})",
                exit.code
            )),
        )
    } else {
        let (value, invalid) = structured(outcome.final_text.as_deref(), request);
        outcome.structured_output = value;
        invalid.map_or((RunStatus::Succeeded, ErrorClass::Ok, None), |message| {
            (RunStatus::Failed, ErrorClass::OutputInvalid, Some(message))
        })
    };
    outcome.status = status;
    outcome.error_class = class;
    outcome.error = error;
    outcome
}

fn failure_class(message: &str) -> ErrorClass {
    let Some(kind) = message
        .strip_prefix(ERROR_PREFIX)
        .and_then(|rest| rest.trim_start().split(':').next())
    else {
        return ErrorClass::HostCrashed;
    };
    if CONNECTOR_KINDS.contains(&kind) {
        ErrorClass::ConnectorUnavailable
    } else if BUDGET_KINDS.contains(&kind) {
        ErrorClass::BudgetExceeded
    } else if OUTPUT_KINDS.contains(&kind) {
        ErrorClass::OutputInvalid
    } else {
        ErrorClass::HostCrashed
    }
}

fn host_error(events: &[RunEventPayload]) -> Option<String> {
    events.iter().find_map(|event| match event {
        RunEventPayload::Runner {
            message,
            level: RunnerLevel::Error,
        } if message.starts_with(DECODER_PREFIX) => Some(message.clone()),
        _ => None,
    })
}

fn session_id(events: &[RunEventPayload]) -> Option<HostSessionId> {
    events.iter().find_map(|event| match event {
        RunEventPayload::Init { session_id, .. } => Some(session_id.clone()),
        _ => None,
    })
}

// JSON: the final answer, shaped by the caller's `RunRequest::output_schema`;
// the second slot is why the answer is missing or invalid.
fn structured(
    final_text: Option<&str>,
    request: &RunRequest,
) -> (Option<serde_json::Value>, Option<String>) {
    if request.output_schema.is_none() {
        return (None, None);
    }
    let Some(text) = final_text else {
        return (None, Some("no final text for the output schema".to_owned()));
    };
    match serde_json::from_str(text.trim()) {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(format!("final text is not JSON: {error}"))),
    }
}
