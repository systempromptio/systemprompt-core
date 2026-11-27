//! Governance telemetry as the governance extension's observer: session
//! moments, new user prompts and finished tool calls reported to the instance
//! as hook events while a manifest plugin owns governance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use harness_engine::extension::GovernorObserver;
use harness_engine::{SessionEvent, ToolCall, ToolOutcome, UserPrompt};

use super::Governance;
use crate::hooks;

/// The governance telemetry: `SessionStart`, `SessionEnd`, `Stop`,
/// `UserPromptSubmit` and `PostToolUse` events for the applied instance,
/// handed to the [`Governance`] event sender it reports through.
pub struct GovernanceTelemetry {
    governance: Arc<Governance>,
}

impl GovernanceTelemetry {
    pub const fn new(governance: Arc<Governance>) -> Self {
        Self { governance }
    }
}

#[async_trait::async_trait]
impl GovernorObserver for GovernanceTelemetry {
    async fn user_prompt(&self, prompt: &UserPrompt) {
        let governance = &self.governance;
        governance.remember_directory(prompt.session_id.as_str(), &prompt.directory);
        governance.report(&prompt.session_id, |_| hooks::prompt(prompt));
    }

    async fn tool_after(&self, call: &ToolCall, outcome: &ToolOutcome) {
        self.governance.report(&call.session_id, |instance| {
            Some(hooks::post_tool(
                call,
                outcome,
                &instance.applied.skill_owners,
            ))
        });
    }

    fn session_event(&self, event: &SessionEvent) {
        let governance = &self.governance;
        match event {
            SessionEvent::Created {
                session_id,
                directory,
                ..
            } => {
                governance.remember_directory(session_id.as_str(), directory);
                governance.report(session_id, |_| {
                    Some(hooks::session_start(session_id, directory))
                });
            },
            SessionEvent::Deleted { session_id } => {
                let directory = governance.forget_directory(session_id.as_str());
                governance.report(session_id, |_| {
                    Some(hooks::session_end(session_id, &directory))
                });
            },
            SessionEvent::Idle { session_id } => {
                let directory = governance.directory(session_id.as_str());
                governance.report(session_id, |_| Some(hooks::stop(session_id, &directory)));
            },
        }
    }
}

impl fmt::Debug for GovernanceTelemetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceTelemetry")
            .field("governance", &self.governance)
            .finish()
    }
}
