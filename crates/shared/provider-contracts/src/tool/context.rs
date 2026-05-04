//! [`ToolContext`] — per-call context forwarded to a
//! [`crate::tool::ToolProvider`].

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub auth_token: String,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub ai_tool_call_id: Option<String>,
    pub headers: HashMap<String, String>,
}

impl ToolContext {
    #[must_use]
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            ai_tool_call_id: None,
            headers: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    #[must_use]
    pub fn with_ai_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.ai_tool_call_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}
