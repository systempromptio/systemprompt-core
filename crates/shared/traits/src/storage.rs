//! File storage trait and identifier types.
//!
//! Defines the [`FileStorage`] abstraction implemented by every storage
//! backend (local disk, object stores, cloud blob services), along with the
//! [`StoredFileId`] / [`StoredFileMetadata`] value types returned across the
//! storage boundary. Errors are reported as [`FileStorageError`] so that
//! callers can match on cause rather than parsing strings.

use async_trait::async_trait;
use std::path::Path;

pub type FileStorageResult<T> = Result<T, FileStorageError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileStorageError {
    #[error("file not found: {0}")]
    NotFound(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("storage backend error: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoredFileId(pub String);

impl StoredFileId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for StoredFileId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for StoredFileId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::fmt::Display for StoredFileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct StoredFileMetadata {
    pub id: StoredFileId,
    pub path: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId>;

    async fn retrieve(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>>;

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()>;

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata>;

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool>;

    fn public_url(&self, _id: &StoredFileId) -> Option<String> {
        None
    }
}
