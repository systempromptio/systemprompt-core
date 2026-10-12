//! Classifies a finished Codex CLI process into a [`RunOutcome`].
//!
//! Precedence: cancelled, then timed out, then a failed turn, then a bare
//! non-zero exit or a missing `turn.completed`, then the structured output
//! check. Tokens and turns are summed over every `Result`; Codex reports no
//! cost, so cost is the gateway's to attribute and stays 0 here. With an
//! output schema, the final `agent_message` is the JSON answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::event::{RunEventPayload, RunnerLevel};
use crate::host::ExitInfo;
use crate::outcome::{ErrorClass, RunOutcome, RunStatus};
use crate::request::RunRequest;

const CONNECTOR_MARKERS: [&str; 5] = [
    "401 Unauthorized",
    "403 Forbidden",
    "Connection failed",
    "error sending request",
    "stream disconnected",
];

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
    let mut failure: Option<String> = None;
    let mut completed = false;
    for event in events {
        if let RunEventPayload::Result {
            is_error,
            input_tokens,
            output_tokens,
            error,
            ..
        } = event
        {
            outcome.input_tokens += input_tokens;
            outcome.output_tokens += output_tokens;
            outcome.turns += 1;
            if *is_error {
                failure.clone_from(error);
            } else {
                completed = true;
            }
        }
    }
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
    } else if let Some(message) = failure {
        (RunStatus::Failed, failure_class(&message), Some(message))
    } else if exit.code != Some(0) || !completed {
        let message =
            last_warning(events).unwrap_or_else(|| format!("codex exited with {:?}", exit.code));
        (RunStatus::Failed, failure_class(&message), Some(message))
    } else {
        let (value, invalid) = structured(events, request);
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
    if CONNECTOR_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
    {
        ErrorClass::ConnectorUnavailable
    } else {
        ErrorClass::HostCrashed
    }
}

fn session_id(events: &[RunEventPayload]) -> Option<systemprompt_identifiers::HostSessionId> {
    events.iter().find_map(|event| match event {
        RunEventPayload::Init { session_id, .. } => Some(session_id.clone()),
        _ => None,
    })
}

fn last_warning(events: &[RunEventPayload]) -> Option<String> {
    events.iter().rev().find_map(|event| match event {
        RunEventPayload::Runner {
            message,
            level: RunnerLevel::Warn | RunnerLevel::Error,
        } => Some(message.clone()),
        _ => None,
    })
}

// JSON: the final answer, shaped by the caller's `RunRequest::output_schema`;
// the second slot is why the answer is missing or invalid.
fn structured(
    events: &[RunEventPayload],
    request: &RunRequest,
) -> (Option<serde_json::Value>, Option<String>) {
    if request.output_schema.is_none() {
        return (None, None);
    }
    let text = events.iter().rev().find_map(|event| match event {
        RunEventPayload::AssistantText { text } => Some(text.as_str()),
        _ => None,
    });
    let Some(text) = text else {
        return (
            None,
            Some("no final agent message for the output schema".to_owned()),
        );
    };
    match serde_json::from_str(text.trim()) {
        Ok(value) => (Some(value), None),
        Err(error) => (
            None,
            Some(format!("final agent message is not JSON: {error}")),
        ),
    }
}
