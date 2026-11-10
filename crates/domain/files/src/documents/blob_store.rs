//! Content-addressed storage for document revision bytes.
//!
//! A blob is stored once per SHA-256 under `documents/blobs/<aa>/<hash>`,
//! outside the `files/` prefix the public file server serves, and has one
//! `document_blobs` row. The row is the authority: a blob without a row is
//! never served, and [`BlobStore::put`] removes an object it just stored
//! when the row cannot be written.
//!
//! [`BlobStore::adopt`] (in `blob_adopt`) registers an object already
//! written elsewhere (an assembled upload) under its verified hash, moving
//! it to the same content-addressed path a `put` uses.
//!
//! Object transfers never run inside a database transaction. `put` and
//! `adopt` write the object first (content-addressed, so idempotent), then
//! take the hash's advisory lock ([`HashLock`]) only to recheck that the
//! object is still there and upsert the row; a collection that removed the
//! object in between makes them store it again under the lock, bounded by
//! [`LOCKED_IO_LIMIT`]. Collection (in `blob_store_gc`) is two short locked
//! steps: delete the row and record the collection, commit; then, if no row
//! came back meanwhile, delete the object (bounded) and the record. A row
//! never exists without its object, and an interrupted collection is
//! finished by the next pass from its record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use systemprompt_identifiers::ContentHash;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_traits::{
    ByteRange, FileStorage, FileStorageError, RepositoryError, StoredFileId, StoredFileRead,
};

use super::models::DocumentBlob;
use super::repository::DocumentBlobRepository;
#[cfg(doc)]
use super::repository::HashLock;

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
    #[error("blob {operation} did not finish within the time allowed under the hash lock")]
    TimedOut {
        operation: &'static str,
        #[source]
        source: tokio::time::error::Elapsed,
    },
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
    pub(super) storage: Arc<dyn FileStorage>,
    pub(super) blobs: Arc<DocumentBlobRepository>,
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
        let mut freshly_stored = if self.storage.exists(&storage_id).await? {
            false
        } else {
            self.storage
                .store(Path::new(storage_id.as_str()), bytes)
                .await?;
            true
        };
        let mut lock = self.blobs.lock(&hash).await?;
        if !self.storage.exists(&storage_id).await? {
            bounded(
                "re-store",
                self.storage.store(Path::new(storage_id.as_str()), bytes),
            )
            .await??;
            freshly_stored = true;
        }
        let inserted = match lock.upsert(&hash, size_column, &storage_id).await {
            Ok(inserted) => inserted,
            Err(error) => {
                self.discard(freshly_stored, &storage_id).await;
                return Err(error.into());
            },
        };
        lock.commit().await?;
        Ok(StoredBlob {
            hash,
            size_bytes: size,
            storage_id,
            inserted,
        })
    }

    pub(super) async fn discard(&self, freshly_stored: bool, storage_id: &StoredFileId) {
        if !freshly_stored {
            return;
        }
        let error = match bounded("discard", self.storage.delete(storage_id)).await {
            Ok(Ok(())) => return,
            Ok(Err(error)) => BlobStoreError::from(error),
            Err(error) => error,
        };
        tracing::error!(
            storage_id = %storage_id,
            error = %error,
            "Failed to remove a blob whose row could not be written"
        );
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
            .open(&StoredFileId::new(row.storage_id.as_str()), range)
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
            .open(&StoredFileId::new(row.storage_id.as_str()), None)
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
}

pub(super) const LOCKED_IO_LIMIT: Duration = Duration::from_secs(30);

pub(super) async fn bounded<T>(
    operation: &'static str,
    work: impl Future<Output = T>,
) -> Result<T, BlobStoreError> {
    tokio::time::timeout(LOCKED_IO_LIMIT, work)
        .await
        .map_err(|source| BlobStoreError::TimedOut { operation, source })
}
