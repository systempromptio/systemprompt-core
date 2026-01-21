use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

#[derive(Debug, Clone)]
pub struct InsertFileRequest {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub metadata: serde_json::Value,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
}

impl InsertFileRequest {
    pub fn new(
        id: FileId,
        path: impl Into<String>,
        public_url: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            id,
            path: path.into(),
            public_url: public_url.into(),
            mime_type: mime_type.into(),
            size_bytes: None,
            ai_content: false,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            user_id: None,
            session_id: None,
            trace_id: None,
            context_id: None,
        }
    }

    pub const fn with_size(mut self, size: i64) -> Self {
        self.size_bytes = Some(size);
        self
    }

    pub const fn with_ai_content(mut self, ai_content: bool) -> Self {
        self.ai_content = ai_content;
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_context_id(mut self, context_id: ContextId) -> Self {
        self.context_id = Some(context_id);
        self
    }
}
