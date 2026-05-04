//! File upload provider trait used by chat surfaces and agent IO.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

/// Result alias for [`FileUploadProvider`] operations.
pub type FileUploadResult<T> = Result<T, FileUploadProviderError>;

/// Errors returned by file upload providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileUploadProviderError {
    /// File uploads are administratively disabled.
    #[error("Upload disabled")]
    Disabled,

    /// The file failed validation (mime, size, name, ...).
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// The backing store rejected or dropped the upload.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for FileUploadProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Builder-style payload describing a file to upload.
#[derive(Debug, Clone)]
pub struct FileUploadInput {
    /// MIME type advertised by the client.
    pub mime_type: String,
    /// Base64-encoded file body.
    pub bytes_base64: String,
    /// Optional logical filename.
    pub name: Option<String>,
    /// Owning context.
    pub context_id: ContextId,
    /// Optional owning user.
    pub user_id: Option<UserId>,
    /// Optional owning session.
    pub session_id: Option<SessionId>,
    /// Optional trace id for observability.
    pub trace_id: Option<TraceId>,
}

impl FileUploadInput {
    /// Construct a new upload payload with the required fields.
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

    /// Attach a logical filename.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Attach an owning user id.
    #[must_use]
    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Attach an owning session id.
    #[must_use]
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Attach a trace id.
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }
}

/// Outcome of a successful file upload.
#[derive(Debug, Clone)]
pub struct UploadedFileInfo {
    /// Backend-assigned file identifier.
    pub file_id: FileId,
    /// Public URL the client can use to fetch the file.
    pub public_url: String,
    /// Stored size in bytes if known.
    pub size_bytes: Option<i64>,
}

/// Persist user-uploaded files.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn FileUploadProvider>` via [`DynFileUploadProvider`].
#[async_trait]
pub trait FileUploadProvider: Send + Sync {
    /// Report whether uploads are currently accepted.
    fn is_enabled(&self) -> bool;

    /// Persist `input` and return the resulting metadata.
    async fn upload_file(&self, input: FileUploadInput) -> FileUploadResult<UploadedFileInfo>;
}

/// Shared `Arc` alias for [`FileUploadProvider`].
pub type DynFileUploadProvider = Arc<dyn FileUploadProvider>;
