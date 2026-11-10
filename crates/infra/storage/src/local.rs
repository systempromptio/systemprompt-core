//! Local-disk [`FileStorage`] backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use futures::TryStreamExt;
use systemprompt_traits::{
    ByteRange, FileByteStream, FileStorage, FileStorageError, FileStorageResult, StoredFileId,
    StoredFileMetadata, StoredFileRead,
};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;

use crate::object_id::{id_for, mime_for, relative_path};

/// Files stored as plain paths under one root directory.
///
/// Ids are root-relative paths. The root itself may be a shared mount, in
/// which case every replica resolves the same id to the same file. A path
/// that passes through a symbolic link below the root is refused, so no id
/// reaches a file outside it.
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

    async fn confined(&self, relative: &Path) -> FileStorageResult<PathBuf> {
        let mut current = self.root.clone();
        for component in relative.components() {
            current.push(component);
            match fs::symlink_metadata(&current).await {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(FileStorageError::Validation(format!(
                        "storage path {} passes through a symbolic link",
                        relative.display()
                    )));
                },
                Ok(_) => {},
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                Err(err) => return Err(err.into()),
            }
        }
        Ok(self.root.join(relative))
    }

    async fn confined_id(&self, id: &StoredFileId) -> FileStorageResult<PathBuf> {
        self.confined(relative_path(Path::new(id.as_str()))?).await
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

fn describe(id: &StoredFileId, full: &Path, meta: &std::fs::Metadata) -> StoredFileMetadata {
    let created_at = meta.created().or_else(|_| meta.modified()).map_or_else(
        |_| chrono::Utc::now(),
        chrono::DateTime::<chrono::Utc>::from,
    );
    let updated_at = meta
        .modified()
        .map_or(created_at, chrono::DateTime::<chrono::Utc>::from);
    StoredFileMetadata {
        id: id.clone(),
        path: id.as_str().to_owned(),
        mime_type: mime_for(full).to_owned(),
        size_bytes: i64::try_from(meta.len()).ok(),
        etag: Some(format!(
            "\"{:x}-{:x}\"",
            meta.len(),
            updated_at.timestamp_nanos_opt().unwrap_or_default()
        )),
        created_at,
        updated_at,
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

fn staging_path(full: &Path) -> PathBuf {
    full.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ))
}

async fn discard(staging: &Path) {
    if let Err(cleanup) = fs::remove_file(staging).await {
        tracing::warn!(path = %staging.display(), error = %cleanup, "staging file left behind");
    }
}

struct StagingGuard(Option<PathBuf>);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(staging) = self.0.take()
            && let Err(error) = std::fs::remove_file(&staging)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(path = %staging.display(), error = %error, "staging file left behind");
        }
    }
}

async fn publish(staging: &Path, full: &Path) -> FileStorageResult<()> {
    if let Err(err) = fs::rename(staging, full).await {
        discard(staging).await;
        return Err(err.into());
    }
    Ok(())
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        let relative = relative_path(path)?;
        let full = self.confined(relative).await?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        // Why: the root may be a mount shared between replicas, and a reader
        // on another replica must never see a half-written file; the rename
        // is atomic on the same filesystem.
        let staging = staging_path(&full);
        let mut file = fs::File::create(&staging).await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        publish(&staging, &full).await?;
        Ok(id_for(relative))
    }

    async fn store_stream(
        &self,
        path: &Path,
        mut body: FileByteStream,
    ) -> FileStorageResult<StoredFileId> {
        use futures::StreamExt;
        let relative = relative_path(path)?;
        let full = self.confined(relative).await?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        let staging = staging_path(&full);
        let mut guard = StagingGuard(Some(staging.clone()));
        let mut file = fs::File::create(&staging).await?;
        while let Some(chunk) = body.next().await {
            file.write_all(&chunk?).await?;
        }
        file.sync_all().await?;
        drop(file);
        publish(&staging, &full).await?;
        guard.0 = None;
        Ok(id_for(relative))
    }

    async fn open(
        &self,
        id: &StoredFileId,
        range: Option<ByteRange>,
    ) -> FileStorageResult<StoredFileRead> {
        let full = self.confined_id(id).await?;
        let mut file = fs::File::open(&full)
            .await
            .map_err(|err| not_found(id, err))?;
        let meta = file.metadata().await?;
        if !meta.is_file() {
            return Err(FileStorageError::NotFound(id.as_str().to_owned()));
        }
        let size = meta.len();
        let served = range.map(|r| r.resolve(size)).transpose()?;
        let (start, len) = served.map_or((0, size), |r| (r.start, r.length()));
        if start > 0 {
            file.seek(std::io::SeekFrom::Start(start)).await?;
        }
        let body = ReaderStream::new(file.take(len)).map_err(FileStorageError::Io);
        Ok(StoredFileRead {
            metadata: describe(id, &full, &meta),
            range: served,
            body: Box::pin(body),
        })
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        let full = self.confined_id(id).await?;
        fs::remove_file(&full)
            .await
            .map_err(|err| not_found(id, err))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let full = self.confined_id(id).await?;
        let meta = fs::metadata(&full)
            .await
            .map_err(|err| not_found(id, err))?;
        if !meta.is_file() {
            return Err(FileStorageError::NotFound(id.as_str().to_owned()));
        }
        Ok(describe(id, &full, &meta))
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        let full = self.confined_id(id).await?;
        Ok(fs::try_exists(&full).await?)
    }
}
