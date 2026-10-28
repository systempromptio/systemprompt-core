//! Registering bytes already written to storage as a blob
//! ([`BlobStore::adopt`]), and locating a blob's object from its row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ContentHash;
use systemprompt_traits::StoredFileId;

use super::blob_store::{BlobStore, BlobStoreError, StoredBlob};

impl BlobStore {
    pub async fn adopt(
        &self,
        hash: &ContentHash,
        size: u64,
        storage_id: &StoredFileId,
    ) -> Result<StoredBlob, BlobStoreError> {
        let size_column = i64::try_from(size)
            .map_err(|source| BlobStoreError::SizeOutOfRange { size, source })?;
        let inserted = self
            .blobs
            .upsert(hash, size_column, storage_id.as_str())
            .await?;
        if inserted {
            return Ok(StoredBlob {
                hash: hash.clone(),
                size_bytes: size,
                storage_id: storage_id.clone(),
                inserted,
            });
        }
        let row = self
            .blobs
            .find_on_primary(hash)
            .await?
            .ok_or_else(|| BlobStoreError::NotFound { hash: hash.clone() })?;
        if row.storage_id != storage_id.as_str()
            && let Err(error) = self.storage.delete(storage_id).await
        {
            tracing::warn!(
                storage_id = %storage_id,
                error = %error,
                "Failed to remove a duplicate adopted blob"
            );
        }
        Ok(StoredBlob {
            hash: hash.clone(),
            size_bytes: row.size_bytes.unsigned_abs(),
            storage_id: StoredFileId::new(row.storage_id),
            inserted,
        })
    }

    /// Where a blob is stored and how large it is, from its row.
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
