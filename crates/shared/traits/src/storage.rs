use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

/// Unique identifier for a stored file
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoredFileId(pub String);

impl StoredFileId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

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

/// Metadata about a stored file
#[derive(Debug, Clone)]
pub struct StoredFileMetadata {
    pub id: StoredFileId,
    pub path: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Trait for file storage operations
///
/// Implementations of this trait handle the actual storage and retrieval
/// of file contents, whether that's local filesystem, cloud storage, etc.
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Store a file at the given path with the provided content
    async fn store(&self, path: &Path, content: &[u8]) -> Result<StoredFileId>;

    /// Retrieve the contents of a file by its ID
    async fn retrieve(&self, id: &StoredFileId) -> Result<Vec<u8>>;

    /// Delete a file by its ID
    async fn delete(&self, id: &StoredFileId) -> Result<()>;

    /// Get metadata for a file by its ID
    async fn metadata(&self, id: &StoredFileId) -> Result<StoredFileMetadata>;

    /// Check if a file exists by its ID
    async fn exists(&self, id: &StoredFileId) -> Result<bool>;

    /// Get the public URL for a file (if applicable)
    fn public_url(&self, id: &StoredFileId) -> Option<String> {
        let _ = id;
        None
    }
}
