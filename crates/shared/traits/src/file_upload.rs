use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

pub type FileUploadResult<T> = Result<T, FileUploadProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileUploadProviderError {
    #[error("Upload disabled")]
    Disabled,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for FileUploadProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct FileUploadInput {
    pub mime_type: String,
    pub bytes_base64: String,
    pub name: Option<String>,
    pub context_id: ContextId,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
}

impl FileUploadInput {
    #[must_use]
    pub fn new(
        mime_type: impl Into<String>,
        bytes_base64: impl Into<String>,
        context_id: ContextId,
    ) -> Self {
        Self {
            mime_type: mime_type.into(),
            bytes_base64: bytes_base64.into(),
            name: None,
            context_id,
            user_id: None,
            session_id: None,
            trace_id: None,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
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
}

#[derive(Debug, Clone)]
pub struct UploadedFileInfo {
    pub file_id: FileId,
    pub public_url: String,
    pub size_bytes: Option<i64>,
}

#[async_trait]
pub trait FileUploadProvider: Send + Sync {
    fn is_enabled(&self) -> bool;

    async fn upload_file(&self, input: FileUploadInput) -> FileUploadResult<UploadedFileInfo>;
}

pub type DynFileUploadProvider = Arc<dyn FileUploadProvider>;
