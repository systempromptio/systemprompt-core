//! Snapshots of a live edit session.
//!
//! A snapshot stores the session's engine state as a blob and prunes the
//! logged updates it covers. The state is sealed while the replica's entry
//! is held; the blob write and the snapshot mark run without it, and one
//! snapshot at a time runs per entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentEditSessionId};

use super::entry::SharedEntry;
use super::{CollabError, CollabService};
use crate::documents::repository::SnapshotMark;

struct Sealed {
    session: DocumentEditSessionId,
    bytes: Vec<u8>,
    version: Vec<u8>,
    seq: i64,
    covers: u32,
}


impl CollabService {
    pub(super) async fn snapshot(&self, shared: &SharedEntry) -> Result<(), CollabError> {
        let Some(sealed) = self.seal(shared).await? else {
            return Ok(());
        };
        let stored = self.store(&sealed).await;
        let mut entry = shared.lock().await;
        entry.snapshotting = false;
        let moved = stored?;
        if let Some(hash) = moved {
            entry.session.snapshot_sha256 = Some(hash);
        }
        entry.unsnapshotted = entry.unsnapshotted.saturating_sub(sealed.covers);
        if entry.unsnapshotted == 0 {
            entry.snapshot_due_from = None;
        }
        drop(entry);
        Ok(())
    }

    async fn seal(&self, shared: &SharedEntry) -> Result<Option<Sealed>, CollabError> {
        if shared.lock().await.snapshotting {
            return Ok(None);
        }
        let mut entry = self.caught_up(shared).await?;
        if entry.snapshotting {
            return Ok(None);
        }
        let sealed = Sealed {
            session: entry.session.id.clone(),
            bytes: entry.images.seal(&entry.doc.snapshot()?),
            version: entry.doc.version(),
            seq: entry.last_seq,
            covers: entry.unsnapshotted,
        };
        entry.snapshotting = true;
        drop(entry);
        Ok(Some(sealed))
    }

    async fn store(&self, sealed: &Sealed) -> Result<Option<ContentHash>, CollabError> {
        let blob = self.documents.blobs().put(&sealed.bytes).await?;
        let moved = self
            .documents
            .repositories()
            .collab
            .set_snapshot(
                &sealed.session,
                SnapshotMark {
                    hash: &blob.hash,
                    seq: sealed.seq,
                    version: &sealed.version,
                },
            )
            .await?;
        Ok(moved.then_some(blob.hash))
    }
}
