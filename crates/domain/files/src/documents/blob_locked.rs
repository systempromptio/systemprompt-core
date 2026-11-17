//! The locked step [`BlobStore::put`] and [`BlobStore::adopt`] share:
//! take the hash's advisory lock, store the object again if a collection
//! removed it, upsert the row, commit.
//!
//! [`HashLock`](super::repository::HashLock) hands its transaction to the
//! caller, so the repository cannot re-run what the caller does inside it;
//! the step is re-run here instead when Postgres aborts it as a deadlock or
//! serialization victim ([`super::transient`]). Re-running is safe: the
//! aborted attempt committed nothing, and the only effect outside the
//! transaction, storing the object, is content-addressed and rechecked under
//! the lock on every attempt. An object this call stored is removed only when
//! the row could not be written on the last attempt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

use systemprompt_identifiers::ContentHash;
use systemprompt_traits::StoredFileId;

use super::blob_store::{BlobStore, BlobStoreError, bounded};
use super::transient::{TransientConflict, retry_conflicts};

#[derive(Debug)]
enum Locked {
    Upsert(BlobStoreError),
    Other(BlobStoreError),
}

impl std::fmt::Display for Locked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upsert(error) | Self::Other(error) => error.fmt(f),
        }
    }
}

impl TransientConflict for Locked {
    fn is_transient_conflict(&self) -> bool {
        match self {
            Self::Upsert(error) | Self::Other(error) => error.is_transient_conflict(),
        }
    }
}

pub(super) struct LockedRow<'a> {
    pub(super) hash: &'a ContentHash,
    pub(super) size_column: i64,
    pub(super) storage_id: &'a StoredFileId,
    pub(super) freshly_stored: bool,
}

impl BlobStore {
    pub(super) async fn register_locked<R, RF>(
        &self,
        row: LockedRow<'_>,
        restore: R,
    ) -> Result<bool, BlobStoreError>
    where
        R: Fn() -> RF + Sync,
        RF: Future<Output = Result<(), BlobStoreError>> + Send,
    {
        let stored = AtomicBool::new(row.freshly_stored);
        let outcome = retry_conflicts(|| self.register_once(&row, &stored, &restore)).await;
        match outcome {
            Ok(inserted) => Ok(inserted),
            Err(Locked::Upsert(error)) => {
                self.discard(stored.load(Ordering::Relaxed), row.storage_id)
                    .await;
                Err(error)
            },
            Err(Locked::Other(error)) => Err(error),
        }
    }

    async fn register_once<R, RF>(
        &self,
        row: &LockedRow<'_>,
        stored: &AtomicBool,
        restore: &R,
    ) -> Result<bool, Locked>
    where
        R: Fn() -> RF + Sync,
        RF: Future<Output = Result<(), BlobStoreError>> + Send,
    {
        let other = |error: BlobStoreError| Locked::Other(error);
        let mut lock = self
            .blobs
            .lock(row.hash)
            .await
            .map_err(|e| other(e.into()))?;
        if !self
            .storage
            .exists(row.storage_id)
            .await
            .map_err(|e| other(e.into()))?
        {
            bounded("re-store", restore())
                .await
                .map_err(other)?
                .map_err(other)?;
            stored.store(true, Ordering::Relaxed);
        }
        let inserted = lock
            .upsert(row.hash, row.size_column, row.storage_id)
            .await
            .map_err(|e| Locked::Upsert(e.into()))?;
        lock.commit().await.map_err(|e| other(e.into()))?;
        Ok(inserted)
    }
}
