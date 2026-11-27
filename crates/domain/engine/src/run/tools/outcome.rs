//! How a tool call ends on its part: completed with the tool's output, or
//! failed, with the marker [`crate::run::denied`] defines when a policy, a
//! rule, the user or an abort stopped it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::prompt::notices;
use crate::store::{Attachment, JsonMap, ToolState, now_ms};
use crate::tools::{ToolError, ToolOutput};
use serde_json::Value;

use super::exec::Invocation;
use super::gate::Gate;
use super::progress::object;
use crate::run::denied;
use crate::run::governor::{ToolCall, ToolOutcome};

pub(crate) struct Finished {
    pub(crate) state: ToolState,
    pub(crate) stop: bool,
}

impl Finished {
    // JSON: the arguments as the model sent them, kept on the stored part.
    pub(crate) fn failed(input: Value, error: String, started: i64) -> Self {
        Self::refused(input, error, started, JsonMap::new())
    }

    // JSON: the arguments as the model sent them, kept on the stored part.
    fn refused(input: Value, error: String, started: i64, metadata: JsonMap) -> Self {
        let state = ToolState::Error {
            input,
            error,
            metadata,
            started,
            ended: now_ms(),
        };
        Self { state, stop: false }
    }
}

// JSON: the validated arguments, kept on the stored part.
pub(super) async fn completed(
    invocation: &Invocation<'_>,
    gate: &Gate,
    governed: &ToolCall,
    input: Value,
    output: ToolOutput,
) -> Finished {
    if !gate.reviewed() {
        tracing::warn!(tool = %governed.tool, "a tool ran without asking the permission gate");
    }
    let outcome = ToolOutcome {
        title: output.title.clone(),
        output: output.output.clone(),
        metadata: output.metadata.clone(),
        error: None,
    };
    invocation
        .ctx
        .inner
        .governed
        .tool_after(governed, &outcome)
        .await;
    let attachments = output
        .attachments
        .iter()
        .map(|attachment| Attachment {
            mime: attachment.mime.clone(),
            name: attachment.filename.clone(),
            url: attachment.data_url(),
        })
        .collect();
    let state = ToolState::Completed {
        input,
        output: output.output,
        title: output.title,
        metadata: object(output.metadata),
        attachments,
        started: invocation.started,
        ended: now_ms(),
    };
    Finished { state, stop: false }
}

// JSON: the arguments as the model sent them, kept on the stored part.
pub(super) async fn failure(
    invocation: &Invocation<'_>,
    gate: &Gate,
    governed: &ToolCall,
    input: Value,
    error: ToolError,
) -> Finished {
    let name = governed.tool.as_str();
    let started = invocation.started;
    if let Some(reason) = gate.policy_denial() {
        let text = notices::tool_denied_by_policy(name, &reason);
        let metadata = denied::marker(denied::POLICY, Some((denied::REASON, reason.as_str())));
        return Finished::refused(input, text, started, metadata);
    }
    let text = error.to_string();
    match &error {
        ToolError::Rejected => {
            let feedback = gate.rejection_feedback();
            let text = notices::tool_rejected_by_user(name, feedback.as_deref());
            let detail = feedback.as_deref().map(|words| (denied::FEEDBACK, words));
            let metadata = denied::marker(denied::REJECTED, detail);
            let stop = feedback.is_none();
            Finished {
                stop,
                ..Finished::refused(input, text, started, metadata)
            }
        },
        ToolError::Denied { reason } if gate.rule_denial() => {
            let metadata = denied::marker(denied::RULE, Some((denied::REASON, reason.as_str())));
            Finished::refused(input, text, started, metadata)
        },
        ToolError::Aborted => {
            let metadata = denied::marker(denied::ABORTED, None);
            Finished::refused(input, text, started, metadata)
        },
        _ => {
            if gate.reviewed() {
                let outcome = ToolOutcome {
                    title: name.to_owned(),
                    output: String::new(),
                    metadata: Value::Null,
                    error: Some(text.clone()),
                };
                invocation
                    .ctx
                    .inner
                    .governed
                    .tool_after(governed, &outcome)
                    .await;
            }
            Finished::failed(input, text, started)
        },
    }
}
