//! Registering bytes already written to storage as a blob
//! ([`BlobStore::adopt`]), and locating a blob's object from its row.
//!
//! Adopting streams the staged object to the blob's content-addressed path
//! (unless that object already exists there), removes the staged copy and
//! upserts the row, so an adopted blob is stored exactly where
//! [`BlobStore::put`] would have stored it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::ContentHash;
use systemprompt_traits::StoredFileId;

use super::blob_store::{BlobStore, BlobStoreError, StoredBlob, storage_id_for};

impl BlobStore {
    pub async fn adopt(
        &self,
        hash: &ContentHash,
        size: u64,
        staged: &StoredFileId,
    ) -> Result<StoredBlob, BlobStoreError> {
        let size_column = i64::try_from(size)
            .map_err(|source| BlobStoreError::SizeOutOfRange { size, source })?;
        let canonical = storage_id_for(hash);
        let copied = if self.storage.exists(&canonical).await? {
            false
        } else {
            let read = self.storage.open(staged, None).await?;
            self.storage
                .store_stream(Path::new(canonical.as_str()), read.body)
                .await?;
            true
        };
        if let Err(error) = self.storage.delete(staged).await {
            tracing::warn!(storage_id = %staged, error = %error, "Failed to remove a staged blob");
        }
        match self
            .blobs
            .upsert(hash, size_column, canonical.as_str())
            .await
        {
            Ok(inserted) => Ok(StoredBlob {
                hash: hash.clone(),
                size_bytes: size,
                storage_id: canonical,
                inserted,
            }),
            Err(error) => {
                if copied && let Err(cleanup) = self.storage.delete(&canonical).await {
                    tracing::error!(
                        storage_id = %canonical,
                        error = %cleanup,
                        "Failed to remove a blob whose row could not be written"
                    );
                }
                Err(error.into())
            },
        }
    }

    pub async fn located(&self, hash: &ContentHash) -> Result<(StoredFileId, u64), BlobStoreError> {
        let row = self
            .blobs
            .find_on_primary(hash)
            .await?
            .ok_or_else(|| BlobStoreError::NotFound { hash: hash.clone() })?;
        Ok((
            StoredFileId::new(row.storage_id),
            row.size_bytes.unsigned_abs(),
        ))
    }

    pub async fn storage_id_of(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<StoredFileId>, BlobStoreError> {
        Ok(self
            .blobs
            .find_on_primary(hash)
            .await?
            .map(|row| StoredFileId::new(row.storage_id)))
    }
}
