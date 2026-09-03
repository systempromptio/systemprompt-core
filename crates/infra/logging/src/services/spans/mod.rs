//! Typed tracing-span constructor carrying request attribution.
//!
//! [`SystemSpan`] wraps a `tracing::Span` seeded with the identifier fields
//! (`user_id`, `session_id`, `trace_id`, and optional `context_id`/`task_id`)
//! that the database log layer extracts to attribute each emitted log row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContextId, TaskId, TraceId};
use tracing::Span;

pub struct SystemSpan(Span);

impl std::fmt::Debug for SystemSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SystemSpan").finish()
    }
}

impl SystemSpan {
    pub fn new(component: &str) -> Self {
        Self(tracing::info_span!(
            "system",
            user_id = "system",
            session_id = "system",
            trace_id = %TraceId::generate().as_str(),
            client_id = %format!("system:{component}"),
            context_id = tracing::field::Empty,
            task_id = tracing::field::Empty,
        ))
    }

    pub fn enter(&self) -> tracing::span::EnteredSpan {
        self.0.clone().entered()
    }

    pub fn record_task_id(&self, task_id: &TaskId) {
        self.0.record("task_id", task_id.as_str());
    }

    pub fn record_context_id(&self, context_id: &ContextId) {
        self.0.record("context_id", context_id.as_str());
    }

    pub const fn span(&self) -> &Span {
        &self.0
    }

    pub fn into_span(self) -> Span {
        self.0
    }
}

impl From<SystemSpan> for Span {
    fn from(system_span: SystemSpan) -> Self {
        system_span.0
    }
}
