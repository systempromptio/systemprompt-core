//! File storage trait and identifier types.
//!
//! Defines the [`FileStorage`] abstraction implemented by every storage
//! backend (local disk, object stores, cloud blob services), along with the
//! [`StoredFileId`] / [`StoredFileMetadata`] value types returned across the
//! storage boundary. Errors are reported as [`FileStorageError`] so that
//! callers can match on cause rather than parsing strings.

use async_trait::async_trait;
use std::path::Path;

/// Result alias for [`FileStorage`] operations.
pub type FileStorageResult<T> = Result<T, FileStorageError>;

/// Errors returned by [`FileStorage`] implementations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileStorageError {
    /// The requested file does not exist in the backend.
    #[error("file not found: {0}")]
    NotFound(String),

    /// The backend rejected the request because of a validation rule
    /// (forbidden path, mime mismatch, size limit, etc).
    #[error("validation failed: {0}")]
    Validation(String),

    /// The underlying I/O subsystem reported a failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization step around metadata persistence failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Catch-all for backend-specific failures that do not map to a more
    /// precise variant.
    #[error("storage backend error: {0}")]
    Backend(String),
}

/// Opaque identifier for a file persisted by a [`FileStorage`] backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoredFileId(pub String);

impl StoredFileId {
    /// Wrap a known string identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the inner identifier as a string slice.
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

/// Metadata describing a file held by a [`FileStorage`] backend.
#[derive(Debug, Clone)]
pub struct StoredFileMetadata {
    /// Backend-assigned identifier for the file.
    pub id: StoredFileId,
    /// Logical path or key the file lives under.
    pub path: String,
    /// MIME type detected at upload time.
    pub mime_type: String,
    /// Size in bytes if the backend exposes it.
    pub size_bytes: Option<i64>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last-modified timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Backend-agnostic file storage interface.
///
/// Implementations persist binary blobs and expose a stable identifier that
/// can be used to retrieve, inspect, and delete the stored content. Trait
/// methods are `async fn` so they can be exercised against remote stores.
///
/// # Errors
/// All fallible methods return [`FileStorageError`]. `NotFound` is reserved
/// for missing-file conditions; transport / I/O failures surface as `Io` or
/// `Backend`.
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Persist `content` keyed under `path` and return the assigned id.
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId>;

    /// Load the bytes previously stored for `id`.
    async fn retrieve(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>>;

    /// Remove the file identified by `id`.
    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()>;

    /// Return metadata for `id` without fetching its content.
    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata>;

    /// Report whether `id` resolves to a stored file.
    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool>;

    /// Return a publicly accessible URL for `id` if the backend supports it.
    fn public_url(&self, _id: &StoredFileId) -> Option<String> {
        None
    }
}
