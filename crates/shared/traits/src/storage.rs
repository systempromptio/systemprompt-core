use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

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
        Self(s.to_string())
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
    async fn store(&self, path: &Path, content: &[u8]) -> Result<StoredFileId>;

    async fn retrieve(&self, id: &StoredFileId) -> Result<Vec<u8>>;

    async fn delete(&self, id: &StoredFileId) -> Result<()>;

    async fn metadata(&self, id: &StoredFileId) -> Result<StoredFileMetadata>;

    async fn exists(&self, id: &StoredFileId) -> Result<bool>;

    fn public_url(&self, _id: &StoredFileId) -> Option<String> {
        None
    }
}
