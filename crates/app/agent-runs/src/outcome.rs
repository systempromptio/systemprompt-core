//! The run lifecycle and its terminal [`RunOutcome`].
//!
//! [`RunStatus`] is the `agent_runs.status` column and [`ErrorClass`] the
//! `agent_runs.error_class` column; both spellings match the table's
//! `CHECK` constraints. [`RunOutcome::final_text`] is the run's last
//! `assistant_text`, the answer an A2A caller reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::HostSessionId;

use crate::macros::text_enum;

text_enum!(
    RunStatus, UnknownStatus {
        Queued => "queued",
        Preparing => "preparing",
        Running => "running",
        Succeeded => "succeeded",
        Failed => "failed",
        Cancelled => "cancelled",
        TimedOut => "timed_out",
    }
);

impl RunStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

text_enum!(
    ErrorClass, UnknownErrorClass {
        Ok => "ok",
        BudgetExceeded => "budget_exceeded",
        Timeout => "timeout",
        OutputInvalid => "output_invalid",
        HookDenied => "hook_denied",
        HostCrashed => "host_crashed",
        ConnectorUnavailable => "connector_unavailable",
    }
);

// JSON: the host's structured final answer, shaped by the caller's
// `RunRequest::output_schema`, stored in `agent_runs.structured_output`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub error_class: ErrorClass,
    pub session_id: Option<HostSessionId>,
    pub cost_microdollars: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub turns: i32,
    pub duration_ms: i64,
    // JSON: shaped by the caller's `RunRequest::output_schema`.
    pub structured_output: Option<serde_json::Value>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    #[serde(default)]
    pub final_text: Option<String>,
}

impl RunOutcome {
    #[must_use]
    pub fn last_assistant_text(events: &[crate::event::RunEventPayload]) -> Option<String> {
        events.iter().rev().find_map(|event| match event {
            crate::event::RunEventPayload::AssistantText { text } => Some(text.clone()),
            _ => None,
        })
    }
}
