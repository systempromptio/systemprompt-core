//! Persistence trait for AI-generated files.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

use super::AiProviderResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGeneratedFile {
    pub id: uuid::Uuid,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl AiGeneratedFile {
    pub fn id(&self) -> FileId {
        FileId::new(self.id.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct InsertAiFileParams {
    pub id: uuid::Uuid,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub metadata: serde_json::Value,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
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
