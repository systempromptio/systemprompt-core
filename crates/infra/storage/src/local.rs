//! Local-disk [`FileStorage`] backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use systemprompt_traits::{
    FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
};
use tokio::fs;
use tokio::io::AsyncWriteExt;

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

pub(crate) fn relative_path(path: &Path) -> Result<&Path, FileStorageError> {
    if path.as_os_str().is_empty() {
        return Err(FileStorageError::Validation(
            "storage path must not be empty".to_owned(),
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {},
            Component::ParentDir => {
                return Err(FileStorageError::Validation(format!(
                    "storage path {} contains a parent-directory component",
                    path.display()
                )));
            },
            Component::RootDir | Component::Prefix(_) => {
                return Err(FileStorageError::Validation(format!(
                    "storage path {} must be relative to the storage root",
                    path.display()
                )));
            },
        }
    }
    Ok(path)
}

fn id_for(path: &Path) -> StoredFileId {
    let normalised: PathBuf = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect();
    StoredFileId::new(normalised.to_string_lossy().replace('\\', "/"))
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("txt" | "md") => "text/plain",
        Some("csv") => "text/csv",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn not_found(id: &StoredFileId, err: &std::io::Error) -> FileStorageError {
    if err.kind() == std::io::ErrorKind::NotFound {
        FileStorageError::NotFound(id.as_str().to_owned())
    } else {
        FileStorageError::Backend(format!("{}: {err}", id.as_str()))
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
        let mut file = fs::File::create(&full).await?;
        file.write_all(content).await?;
        file.flush().await?;
        Ok(id_for(relative))
    }

    async fn retrieve(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
        let full = self.resolve(id)?;
        fs::read(&full).await.map_err(|err| not_found(id, &err))
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        let full = self.resolve(id)?;
        fs::remove_file(&full)
            .await
            .map_err(|err| not_found(id, &err))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let full = self.resolve(id)?;
        let meta = fs::metadata(&full)
            .await
            .map_err(|err| not_found(id, &err))?;
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
