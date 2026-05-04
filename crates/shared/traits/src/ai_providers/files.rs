//! Persistence trait for AI-generated files.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

use super::AiProviderResult;

/// Persisted record describing a single AI-generated file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGeneratedFile {
    /// Database UUID for the file.
    pub id: uuid::Uuid,
    /// Filesystem-like path the file is stored at.
    pub path: String,
    /// Public URL for the file.
    pub public_url: String,
    /// MIME type.
    pub mime_type: String,
    /// File size in bytes if known.
    pub size_bytes: Option<i64>,
    /// Whether the file was produced by AI (always `true` here).
    pub ai_content: bool,
    /// Free-form metadata blob.
    pub metadata: serde_json::Value,
    /// Owning user.
    pub user_id: Option<UserId>,
    /// Owning session.
    pub session_id: Option<SessionId>,
    /// Trace id at generation time.
    pub trace_id: Option<TraceId>,
    /// Owning context.
    pub context_id: Option<ContextId>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-modified timestamp.
    pub updated_at: DateTime<Utc>,
    /// Soft-delete timestamp.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl AiGeneratedFile {
    /// Return the typed [`FileId`] for the file.
    pub fn id(&self) -> FileId {
        FileId::new(self.id.to_string())
    }
}

/// Insert payload for [`AiFilePersistenceProvider::insert_file`].
#[derive(Debug, Clone)]
pub struct InsertAiFileParams {
    /// New row's UUID.
    pub id: uuid::Uuid,
    /// Storage path.
    pub path: String,
    /// Public URL.
    pub public_url: String,
    /// MIME type.
    pub mime_type: String,
    /// File size in bytes if known.
    pub size_bytes: Option<i64>,
    /// Free-form metadata blob.
    pub metadata: serde_json::Value,
    /// Owning user.
    pub user_id: Option<UserId>,
    /// Owning session.
    pub session_id: Option<SessionId>,
    /// Trace id at generation time.
    pub trace_id: Option<TraceId>,
    /// Owning context.
    pub context_id: Option<ContextId>,
}

/// Filesystem and URL prefix configuration for the AI image store.
#[derive(Debug, Clone)]
pub struct ImageStorageConfig {
    /// Filesystem root for AI-generated content.
    pub base_path: PathBuf,
    /// URL prefix that maps onto [`Self::base_path`].
    pub url_prefix: String,
}

/// Persistence layer for AI-generated files.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn AiFilePersistenceProvider>` via [`DynAiFilePersistenceProvider`].
#[async_trait]
pub trait AiFilePersistenceProvider: Send + Sync {
    /// Insert a new generated-file row.
    async fn insert_file(&self, params: InsertAiFileParams) -> AiProviderResult<()>;

    /// Look up a row by id.
    async fn find_by_id(&self, id: &FileId) -> AiProviderResult<Option<AiGeneratedFile>>;

    /// List rows owned by `user_id`, paginated.
    async fn list_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> AiProviderResult<Vec<AiGeneratedFile>>;

    /// Soft-delete a row.
    async fn delete(&self, id: &FileId) -> AiProviderResult<()>;

    /// Return the configured storage roots.
    fn storage_config(&self) -> AiProviderResult<ImageStorageConfig>;
}

/// Shared `Arc` alias for [`AiFilePersistenceProvider`].
pub type DynAiFilePersistenceProvider = Arc<dyn AiFilePersistenceProvider>;
