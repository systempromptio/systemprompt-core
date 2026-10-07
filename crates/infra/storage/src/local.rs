//! Local-disk [`FileStorage`] backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use systemprompt_traits::{
    FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::object_id::{id_for, mime_for, relative_path};

/// Files stored as plain paths under one root directory.
///
/// Ids are root-relative paths. The root itself may be a shared mount, in
/// which case every replica resolves the same id to the same file.
#[derive(Debug, Clone)]
pub struct LocalFileStorage {
    root: PathBuf,
}

impl LocalFileStorage {
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, id: &StoredFileId) -> Result<PathBuf, FileStorageError> {
        let relative = relative_path(Path::new(id.as_str()))?;
        Ok(self.root.join(relative))
    }
}

#[derive(Debug)]
struct StoredFileIoError {
    id: StoredFileId,
    source: std::io::Error,
}

impl std::fmt::Display for StoredFileIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stored file {}", self.id)
    }
}

impl std::error::Error for StoredFileIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

fn not_found(id: &StoredFileId, err: std::io::Error) -> FileStorageError {
    if err.kind() == std::io::ErrorKind::NotFound {
        FileStorageError::NotFound(id.as_str().to_owned())
    } else {
        FileStorageError::Backend(Box::new(StoredFileIoError {
            id: id.clone(),
            source: err,
        }))
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        let relative = relative_path(path)?;
        let full = self.root.join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        // Why: the root may be a mount shared between replicas, and a reader
        // on another replica must never see a half-written file; the rename
        // is atomic on the same filesystem.
        let staging = full.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let mut file = fs::File::create(&staging).await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        if let Err(err) = fs::rename(&staging, &full).await {
            if let Err(cleanup) = fs::remove_file(&staging).await {
                tracing::warn!(path = %staging.display(), error = %cleanup, "staging file left behind");
            }
            return Err(err.into());
        }
        Ok(id_for(relative))
    }

    async fn retrieve(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
        let full = self.resolve(id)?;
        fs::read(&full).await.map_err(|err| not_found(id, err))
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        let full = self.resolve(id)?;
        fs::remove_file(&full)
            .await
            .map_err(|err| not_found(id, err))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let full = self.resolve(id)?;
        let meta = fs::metadata(&full)
            .await
            .map_err(|err| not_found(id, err))?;
        let created_at = meta.created().or_else(|_| meta.modified()).map_or_else(
            |_| chrono::Utc::now(),
            chrono::DateTime::<chrono::Utc>::from,
        );
        let updated_at = meta
            .modified()
            .map_or(created_at, chrono::DateTime::<chrono::Utc>::from);
        Ok(StoredFileMetadata {
            id: id.clone(),
            path: id.as_str().to_owned(),
            mime_type: mime_for(&full).to_owned(),
            size_bytes: i64::try_from(meta.len()).ok(),
            created_at,
            updated_at,
        })
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        let full = self.resolve(id)?;
        Ok(fs::try_exists(&full).await?)
    }
}
