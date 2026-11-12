//! Garbage collection of unreferenced blobs, in two locked steps per hash so
//! that no object transfer runs inside a transaction that also changes rows
//! for long (see `blob_store`). Each pass first finishes collections an
//! earlier pass recorded but did not complete.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ContentHash;
use systemprompt_traits::{FileStorageError, StoredFileId};

use super::blob_store::{BlobStore, BlobStoreError, GcReport, bounded};

impl BlobStore {
    pub async fn delete_unreferenced(
        &self,
        older_than_hours: u32,
        limit: u32,
    ) -> Result<GcReport, BlobStoreError> {
        let mut report = GcReport::default();
        for (hash, storage_id) in self.blobs.pending_collections(i64::from(limit)).await? {
            count(
                &mut report,
                &hash,
                self.finish_collection(&hash, &storage_id).await,
            );
        }
        let candidates = self
            .blobs
            .unreferenced_older_than(i64::from(older_than_hours), i64::from(limit))
            .await?;
        for blob in candidates {
            report.examined += 1;
            let storage_id = StoredFileId::new(blob.storage_id.as_str());
            let outcome = match self
                .start_collection(&blob.content_sha256, &storage_id)
                .await
            {
                Ok(true) => {
                    self.finish_collection(&blob.content_sha256, &storage_id)
                        .await
                },
                other => other,
            };
            count(&mut report, &blob.content_sha256, outcome);
        }
        Ok(report)
    }

    pub async fn start_collection(
        &self,
        hash: &ContentHash,
        storage_id: &StoredFileId,
    ) -> Result<bool, BlobStoreError> {
        let mut lock = self.blobs.lock(hash).await?;
        let started = lock.begin_collection(hash, storage_id).await?;
        lock.commit().await?;
        Ok(started)
    }

    pub async fn finish_collection(
        &self,
        hash: &ContentHash,
        storage_id: &StoredFileId,
    ) -> Result<bool, BlobStoreError> {
        let mut lock = self.blobs.lock(hash).await?;
        let collected = if lock.row_exists(hash).await? {
            false
        } else {
            match bounded("delete", self.storage.delete(storage_id)).await? {
                Ok(()) | Err(FileStorageError::NotFound(_)) => true,
                Err(error) => return Err(error.into()),
            }
        };
        lock.end_collection(hash).await?;
        lock.commit().await?;
        Ok(collected)
    }
}

fn count(report: &mut GcReport, hash: &ContentHash, outcome: Result<bool, BlobStoreError>) {
    match outcome {
        Ok(true) => report.deleted += 1,
        Ok(false) => {},
        Err(error) => {
            tracing::warn!(
                content_sha256 = %hash,
                error = %error,
                "Failed to collect an unreferenced blob"
            );
            report.failed += 1;
        },
    }
}
