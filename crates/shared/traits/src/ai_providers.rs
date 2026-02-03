use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, SessionId, SessionSource, TraceId, UserId};

pub type AiProviderResult<T> = Result<T, AiProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiProviderError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AiProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<ImageGenerationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationInfo {
    pub prompt: String,
    pub model: String,
    pub provider: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_time_ms: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_estimate: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ImageMetadata {
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            alt_text: None,
            description: None,
            generation: None,
        }
    }

    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_alt_text(mut self, alt: impl Into<String>) -> Self {
        self.alt_text = Some(alt.into());
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_generation(mut self, gen: ImageGenerationInfo) -> Self {
        self.generation = Some(gen);
        self
    }
}

impl ImageGenerationInfo {
    pub fn new(
        prompt: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            model: model.into(),
            provider: provider.into(),
            resolution: None,
            aspect_ratio: None,
            generation_time_ms: None,
            cost_estimate: None,
            request_id: None,
        }
    }

    pub fn with_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }

    pub fn with_aspect_ratio(mut self, aspect_ratio: impl Into<String>) -> Self {
        self.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    pub const fn with_generation_time(mut self, time_ms: i32) -> Self {
        self.generation_time_ms = Some(time_ms);
        self
    }

    pub const fn with_cost_estimate(mut self, cost: f32) -> Self {
        self.cost_estimate = Some(cost);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

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

#[derive(Debug, Clone)]
pub struct CreateAiSessionParams<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub session_source: SessionSource,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AiSessionProvider: Send + Sync {
    async fn session_exists(&self, session_id: &SessionId) -> AiProviderResult<bool>;

    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()>;

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()>;
}

pub type DynAiFilePersistenceProvider = Arc<dyn AiFilePersistenceProvider>;
pub type DynAiSessionProvider = Arc<dyn AiSessionProvider>;
