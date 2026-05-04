use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

use super::metadata::FileMetadata;
use crate::error::FilesResult;

/// Database row representation of a file plus metadata, identifiers, and
/// timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct File {
    /// Primary key.
    pub id: uuid::Uuid,
    /// Absolute filesystem path of the stored file.
    pub path: String,
    /// Public URL the file is served from.
    pub public_url: String,
    /// MIME type detected at ingestion or upload time.
    pub mime_type: String,
    /// File size in bytes (None when the size could not be determined).
    pub size_bytes: Option<i64>,
    /// True when the file was produced by an AI model (e.g. generated image).
    pub ai_content: bool,
    /// Free-form structured metadata; deserialise via [`File::metadata`].
    pub metadata: serde_json::Value,
    /// Owning user, if any.
    pub user_id: Option<UserId>,
    /// Owning session, if any.
    pub session_id: Option<SessionId>,
    /// Originating trace, if any.
    pub trace_id: Option<TraceId>,
    /// Originating context, if any.
    pub context_id: Option<ContextId>,
    /// Row creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Row last-update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Soft-delete tombstone timestamp.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl File {
    /// Returns the typed [`FileId`] derived from the row UUID.
    pub fn id(&self) -> FileId {
        FileId::new(self.id.to_string())
    }

    /// Deserialises the structured `metadata` JSON column into a typed
    /// [`FileMetadata`].
    pub fn metadata(&self) -> FilesResult<FileMetadata> {
        serde_json::from_value(self.metadata.clone()).map_err(Into::into)
    }
}
