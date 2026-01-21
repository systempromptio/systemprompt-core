use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

#[derive(Debug, Clone)]
pub struct FileUploadRequest {
    pub name: Option<String>,
    pub mime_type: String,
    pub bytes_base64: String,
    pub context_id: ContextId,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug)]
pub struct FileUploadRequestBuilder {
    mime_type: String,
    bytes_base64: String,
    context_id: ContextId,
    name: Option<String>,
    user_id: Option<UserId>,
    session_id: Option<SessionId>,
    trace_id: Option<TraceId>,
}

impl FileUploadRequestBuilder {
    pub fn new(
        mime_type: impl Into<String>,
        bytes_base64: impl Into<String>,
        context_id: ContextId,
    ) -> Self {
        Self {
            mime_type: mime_type.into(),
            bytes_base64: bytes_base64.into(),
            context_id,
            name: None,
            user_id: None,
            session_id: None,
            trace_id: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
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

    pub fn build(self) -> FileUploadRequest {
        FileUploadRequest {
            name: self.name,
            mime_type: self.mime_type,
            bytes_base64: self.bytes_base64,
            context_id: self.context_id,
            user_id: self.user_id,
            session_id: self.session_id,
            trace_id: self.trace_id,
        }
    }
}

impl FileUploadRequest {
    pub fn builder(
        mime_type: impl Into<String>,
        bytes_base64: impl Into<String>,
        context_id: ContextId,
    ) -> FileUploadRequestBuilder {
        FileUploadRequestBuilder::new(mime_type, bytes_base64, context_id)
    }
}

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub file_id: FileId,
    pub path: String,
    pub public_url: String,
    pub size_bytes: i64,
}
