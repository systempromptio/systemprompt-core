//! Classifies a finished Claude Code process into a [`RunOutcome`].
//!
//! Precedence: cancelled, then timed out, then a governance hook deny, then
//! the final `result` event's subtype, then a bare non-zero exit. Tokens,
//! cost, turns and duration come from the last `result` payload; the session
//! id from it or, failing that, from the `init` payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::HostSessionId;

use crate::event::{HookDecision, RunEventPayload};
use crate::host::ExitInfo;
use crate::outcome::{ErrorClass, RunOutcome, RunStatus};
use crate::request::RunRequest;

const SUCCESS: &str = "success";
const MAX_TURNS: &str = "error_max_turns";
const MAX_BUDGET: &str = "error_max_budget_usd";
const MAX_STRUCTURED_RETRIES: &str = "error_max_structured_output_retries";

struct Verdict {
    status: RunStatus,
    class: ErrorClass,
    error: Option<String>,
}

const fn verdict(status: RunStatus, class: ErrorClass, error: Option<String>) -> Verdict {
    Verdict {
        status,
        class,
        error,
    }
}

fn last_result(events: &[RunEventPayload]) -> Option<&RunEventPayload> {
    events
        .iter()
        .rev()
        .find(|event| matches!(event, RunEventPayload::Result { .. }))
}

fn init_session(events: &[RunEventPayload]) -> Option<HostSessionId> {
    events.iter().find_map(|event| match event {
        RunEventPayload::Init { session_id, .. } => Some(session_id.clone()),
        _ => None,
    })
}

fn denied_hook(events: &[RunEventPayload]) -> Option<String> {
    events.iter().find_map(|event| match event {
        RunEventPayload::Hook {
            name,
            decision: HookDecision::Deny,
            policy,
        } => Some(policy.as_ref().map_or_else(
            || format!("hook {name} denied"),
            |policy| format!("hook {name} denied by policy {policy}"),
        )),
        _ => None,
    })
}

#[must_use]
pub fn classify(exit: ExitInfo, events: &[RunEventPayload], request: &RunRequest) -> RunOutcome {
    let result = last_result(events);
    let verdict = judge(exit, events, result, request);
    let mut outcome = RunOutcome {
        status: verdict.status,
        error_class: verdict.class,
        session_id: init_session(events),
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 0,
        duration_ms: exit.duration_ms,
        structured_output: None,
        exit_code: exit.code,
        error: verdict.error,
    };
    if let Some(RunEventPayload::Result {
        num_turns,
        duration_ms,
        input_tokens,
        output_tokens,
        cost_microdollars,
        structured_output,
        ..
    }) = result
    {
        outcome.cost_microdollars = *cost_microdollars;
        outcome.input_tokens = *input_tokens;
        outcome.output_tokens = *output_tokens;
        outcome.turns = i32::try_from(*num_turns).unwrap_or(i32::MAX);
        if *duration_ms > 0 {
            outcome.duration_ms = *duration_ms;
        }
        outcome.structured_output.clone_from(structured_output);
    }
    outcome
}

fn judge(
    exit: ExitInfo,
    events: &[RunEventPayload],
    result: Option<&RunEventPayload>,
    request: &RunRequest,
) -> Verdict {
    if exit.cancelled {
        return verdict(
            RunStatus::Cancelled,
            ErrorClass::Ok,
            Some("cancelled".to_owned()),
        );
    }
    if exit.timed_out {
        return verdict(
            RunStatus::TimedOut,
            ErrorClass::Timeout,
            Some("timed out".to_owned()),
        );
    }
    if let Some(reason) = denied_hook(events) {
        return verdict(RunStatus::Failed, ErrorClass::HookDenied, Some(reason));
    }
    let Some(RunEventPayload::Result {
        subtype,
        is_error,
        structured_output,
        error,
        ..
    }) = result
    else {
        let reason = format!("claude exited ({:?}) without a result event", exit.code);
        return verdict(RunStatus::Failed, ErrorClass::HostCrashed, Some(reason));
    };
    let reason = || Some(error.clone().unwrap_or_else(|| subtype.clone()));
    match subtype.as_str() {
        SUCCESS if !*is_error => {
            if request.output_schema.is_some() && structured_output.is_none() {
                let missing = "output schema requested but no structured output returned";
                verdict(
                    RunStatus::Failed,
                    ErrorClass::OutputInvalid,
                    Some(missing.to_owned()),
                )
            } else {
                verdict(RunStatus::Succeeded, ErrorClass::Ok, None)
            }
        },
        MAX_TURNS | MAX_BUDGET => verdict(RunStatus::Failed, ErrorClass::BudgetExceeded, reason()),
        MAX_STRUCTURED_RETRIES => verdict(RunStatus::Failed, ErrorClass::OutputInvalid, reason()),
        _ => verdict(RunStatus::Failed, ErrorClass::HostCrashed, reason()),
    }
}
