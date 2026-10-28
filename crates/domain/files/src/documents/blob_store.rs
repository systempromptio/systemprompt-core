//! Content-addressed storage for document revision bytes.
//!
//! A blob is stored once per SHA-256 under `documents/blobs/<aa>/<hash>`,
//! outside the `files/` prefix the public file server serves, and has one
//! `document_blobs` row. The row is the authority: a blob without a row is
//! never served, and [`BlobStore::put`] removes an object it just stored
//! when the row cannot be written.
//!
//! Garbage collection deletes the row first, because the repository refuses
//! to delete a row a revision or edit session still references; only then is
//! the object removed. A failed object delete leaves an orphaned object and
//! no row. The orphan is never served, and a later `put` of the same bytes
//! finds it and restores the row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use futures_util::StreamExt;
use systemprompt_identifiers::ContentHash;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_traits::{
    ByteRange, FileStorage, FileStorageError, RepositoryError, StoredFileId, StoredFileRead,
};

use super::models::DocumentBlob;
use super::repository::DocumentBlobRepository;

#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    #[error("blob storage failed")]
    Storage(#[from] FileStorageError),
    #[error("blob repository failed")]
    Repository(#[from] RepositoryError),
    #[error("blob {hash} does not exist")]
    NotFound { hash: ContentHash },
    #[error("blob of {size} bytes exceeds the {limit}-byte limit")]
    TooLarge { size: u64, limit: u64 },
    #[error("blob of {size} bytes does not fit the size column")]
    SizeOutOfRange {
        size: u64,
        #[source]
        source: std::num::TryFromIntError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
    pub hash: ContentHash,
    pub size_bytes: u64,
    pub storage_id: StoredFileId,
    pub inserted: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcReport {
    pub examined: u64,
    pub deleted: u64,
    pub failed: u64,
}

#[derive(Clone)]
pub struct BlobStore {
    storage: Arc<dyn FileStorage>,
    blobs: Arc<DocumentBlobRepository>,
}

impl std::fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobStore").finish_non_exhaustive()
    }
}

#[must_use]
pub fn storage_id_for(hash: &ContentHash) -> StoredFileId {
    let hex = hash.as_str();
    let shard = hex.get(..2).unwrap_or(hex);
    StoredFileId::new(format!("documents/blobs/{shard}/{hex}"))
}

impl BlobStore {
    pub fn new(storage: Arc<dyn FileStorage>, blobs: Arc<DocumentBlobRepository>) -> Self {
        Self { storage, blobs }
    }

    pub async fn put(&self, bytes: &[u8]) -> Result<StoredBlob, BlobStoreError> {
        let hash = content_hash(bytes);
        let storage_id = storage_id_for(&hash);
        let size = bytes.len() as u64;
        let size_column = i64::try_from(size)
            .map_err(|source| BlobStoreError::SizeOutOfRange { size, source })?;

        let freshly_stored = if self.storage.exists(&storage_id).await? {
            false
        } else {
            self.storage
                .store(Path::new(storage_id.as_str()), bytes)
                .await?;
            true
        };

        match self
            .blobs
            .upsert(&hash, size_column, storage_id.as_str())
            .await
        {
            Ok(inserted) => Ok(StoredBlob {
                hash,
                size_bytes: size,
                storage_id,
                inserted,
            }),
            Err(error) => {
                if freshly_stored && let Err(cleanup) = self.storage.delete(&storage_id).await {
                    tracing::error!(
                        storage_id = %storage_id,
                        error = %cleanup,
                        "Failed to remove a blob whose row could not be written"
                    );
                }
                Err(error.into())
            },
        }
    }

    pub async fn open(
        &self,
        hash: &ContentHash,
        range: Option<ByteRange>,
    ) -> Result<StoredFileRead, BlobStoreError> {
        let row = self
            .blobs
            .find(hash)
            .await?
            .ok_or_else(|| BlobStoreError::NotFound { hash: hash.clone() })?;
        Ok(self
            .storage
            .open(&StoredFileId::new(row.storage_id), range)
            .await?)
    }

    pub async fn read_all(
        &self,
        hash: &ContentHash,
        max_bytes: u64,
    ) -> Result<Vec<u8>, BlobStoreError> {
        let row = self.blobs.find(hash).await?;
        self.read_row(hash, row, max_bytes).await
    }

    pub async fn read_all_primary(
        &self,
        hash: &ContentHash,
        max_bytes: u64,
    ) -> Result<Vec<u8>, BlobStoreError> {
        let row = self.blobs.find_on_primary(hash).await?;
        self.read_row(hash, row, max_bytes).await
    }

    async fn read_row(
        &self,
        hash: &ContentHash,
        row: Option<DocumentBlob>,
        max_bytes: u64,
    ) -> Result<Vec<u8>, BlobStoreError> {
        let row = row.ok_or_else(|| BlobStoreError::NotFound { hash: hash.clone() })?;
        let declared = row.size_bytes.unsigned_abs();
        if declared > max_bytes {
            return Err(BlobStoreError::TooLarge {
                size: declared,
                limit: max_bytes,
            });
        }
        let mut read = self
            .storage
            .open(&StoredFileId::new(row.storage_id), None)
            .await?;
        let mut bytes = Vec::with_capacity(usize::try_from(declared).unwrap_or(0));
        while let Some(chunk) = read.body.next().await {
            bytes.extend_from_slice(&chunk?);
            let read_so_far = bytes.len() as u64;
            if read_so_far > max_bytes {
                return Err(BlobStoreError::TooLarge {
                    size: read_so_far,
                    limit: max_bytes,
                });
            }
        }
        Ok(bytes)
    }

    pub async fn delete_unreferenced(
        &self,
        older_than_hours: u32,
        limit: u32,
    ) -> Result<GcReport, BlobStoreError> {
        let candidates = self
            .blobs
            .unreferenced_older_than(i64::from(older_than_hours), i64::from(limit))
            .await?;
        let mut report = GcReport::default();
        for blob in candidates {
            report.examined += 1;
            match self.blobs.delete(&blob.content_sha256).await {
                Ok(true) => {},
                Ok(false) => continue,
                Err(error) => {
                    tracing::warn!(
                        content_sha256 = %blob.content_sha256,
                        error = %error,
                        "Failed to delete an unreferenced blob row"
                    );
                    report.failed += 1;
                    continue;
                },
            }
            let storage_id = StoredFileId::new(blob.storage_id);
            match self.storage.delete(&storage_id).await {
                Ok(()) | Err(FileStorageError::NotFound(_)) => report.deleted += 1,
                Err(error) => {
                    tracing::warn!(
                        content_sha256 = %blob.content_sha256,
                        storage_id = %storage_id,
                        error = %error,
                        "Deleted a blob row but failed to delete its object"
                    );
                    report.failed += 1;
                },
            }
        }
        Ok(report)
    }
}
