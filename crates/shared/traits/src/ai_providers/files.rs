//! Persistence trait for AI-generated files.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

use super::AiProviderResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGeneratedFile {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    // JSON: free-form per-file metadata bag
    pub metadata: serde_json::Value,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct InsertAiFileParams {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    // JSON: free-form per-file metadata bag
    pub metadata: serde_json::Value,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
}

impl InsertAiFileParams {
    #[must_use]
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
            metadata: serde_json::Value::Null,
            user_id: None,
            session_id: None,
            trace_id: None,
            context_id: None,
        }
    }

    #[must_use]
    pub const fn with_size_bytes(mut self, size_bytes: Option<i64>) -> Self {
        self.size_bytes = size_bytes;
        self
    }

    #[must_use]
    // JSON: Provider file metadata persisted to `ai_files.metadata` (JSONB).
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    #[must_use]
    pub fn with_user_id(mut self, user_id: Option<UserId>) -> Self {
        self.user_id = user_id;
        self
    }

    #[must_use]
    pub fn with_session_id(mut self, session_id: Option<SessionId>) -> Self {
        self.session_id = session_id;
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, trace_id: Option<TraceId>) -> Self {
        self.trace_id = trace_id;
        self
    }

    #[must_use]
    pub fn with_context_id(mut self, context_id: Option<ContextId>) -> Self {
        self.context_id = context_id;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ImageStorageConfig {
    pub base_path: PathBuf,
    pub url_prefix: String,
}

#[async_trait]
pub trait AiFilePersistenceProvider: Send + Sync {
    async fn insert_file(&self, params: InsertAiFileParams) -> AiProviderResult<()>;

    async fn find_by_id(&self, id: &FileId) -> AiProviderResult<Option<AiGeneratedFile>>;

    async fn list_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> AiProviderResult<Vec<AiGeneratedFile>>;

    async fn delete(&self, id: &FileId) -> AiProviderResult<()>;

    fn storage_config(&self) -> AiProviderResult<ImageStorageConfig>;
}

pub type DynAiFilePersistenceProvider = Arc<dyn AiFilePersistenceProvider>;
