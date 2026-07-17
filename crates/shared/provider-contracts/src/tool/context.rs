//! [`ToolContext`] — per-call context forwarded to a
//! [`crate::tool::ToolProvider`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{Actor, AiToolCallId, SessionId, TraceId};

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub actor: Actor,
    pub auth_token: String,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub headers: HashMap<String, String>,
}

impl ToolContext {
    #[must_use]
    pub fn new(actor: Actor, auth_token: impl Into<String>) -> Self {
        Self {
            actor,
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            ai_tool_call_id: None,
            headers: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    #[must_use]
    pub fn with_ai_tool_call_id(mut self, id: AiToolCallId) -> Self {
        self.ai_tool_call_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}
