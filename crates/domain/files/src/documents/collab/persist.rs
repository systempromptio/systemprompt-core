//! Snapshots and checkpoints of a live edit session.
//!
//! A snapshot stores the session's engine state as a blob and prunes the
//! logged updates it covers. The state is sealed while the replica's entry
//! is held; the blob write and the snapshot mark run without it, and one
//! snapshot at a time runs per entry. A checkpoint materialises the live model
//! and commits it as a revision with origin `edit_session`, based on the
//! session's previous checkpoint (or its base revision); when the revision
//! engine merges it with outside writes, the merged document is re-imported
//! with the live block ids carried over and written back into the live
//! model, logged and broadcast like any other update.
//!
//! A checkpoint runs under the session's checkpoint lease: the lease claim
//! re-reads the session row, so the checkpoint is based on the base the last
//! checkpoint on any replica recorded, and one replica at a time checkpoints
//! a session. The claim also reads who wrote since that checkpoint and the
//! actors they ran as (see the sibling `committer` module), so the
//! checkpoint's author and actor never depend on which replica runs it. The
//! lease is renewed while the checkpoint runs; a renewal that finds the
//! lease gone abandons the checkpoint before it records anything. The lease
//! poll and the commit run without holding the replica's session entry, so
//! updates keep being applied while a revision lands.
//!
//! Settling a session that is being left or closed lands a conflicting body
//! as a proposal on the checkpoint base, authored by the committer, then
//! writes the head into the live model and records the head as the
//! checkpoint base, so the same body is never proposed twice and no live
//! edit is lost. The session closes only when no update was logged after
//! the settled body; otherwise it is settled again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentEditSessionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::committer::{Committer, fold_logged};
use super::entry::SharedEntry;
use super::lease::{ClaimWait, Fence, SessionKey, SettledBy};
use super::persist_commit::Rebase;
use super::{Checkpoint, CollabError, CollabService, Left};
use crate::documents::model::EditLevel;
use crate::documents::repository::{CheckpointLease, SnapshotMark};

const SETTLE_ATTEMPTS: u32 = 3;

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

    pub(super) async fn checkpoint(
        &self,
        shared: &SharedEntry,
        trace: &TraceId,
    ) -> Result<Option<Checkpoint>, CollabError> {
        let key = SessionKey::of(&*shared.lock().await);
        let lease = CheckpointLease::generate();
        let fence = Fence {
            key: &key,
            lease: &lease,
            trace,
            by: SettledBy::Participant,
        };
        let Some(committer) = self.claim(shared, &fence, ClaimWait::No, None).await? else {
            return Ok(None);
        };
        let outcome = match committer {
            Committer::Editor(editor) => {
                let work = self.checkpoint_leased(shared, &editor, &fence);
                self.fenced(&key, &lease, Box::pin(work)).await
            },
            Committer::Commenter(_) | Committer::Nobody => {
                shared.lock().await.checkpoint_due_from = None;
                Ok(Checkpoint::Unchanged)
            },
        };
        self.release(&key.session, &lease).await;
        outcome.map(Some)
    }

    pub(super) async fn settle(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<Left, CollabError> {
        self.settle_as(shared, Some(who), trace, SettledBy::Participant)
            .await
    }

    pub(super) async fn settle_as(
        &self,
        shared: &SharedEntry,
        known: Option<&AuthenticatedUser>,
        trace: &TraceId,
        by: SettledBy,
    ) -> Result<Left, CollabError> {
        let key = SessionKey::of(&*shared.lock().await);
        let lease = CheckpointLease::generate();
        let fence = Fence {
            key: &key,
            lease: &lease,
            trace,
            by,
        };
        let Some(committer) = self
            .claim(shared, &fence, ClaimWait::Bounded, known)
            .await?
        else {
            return Err(key.lease_lost());
        };
        let work = self.settle_leased(shared, &fence, committer);
        let outcome = self.fenced(&key, &lease, Box::pin(work)).await;
        if !matches!(outcome, Ok(Left { closed: true, .. })) {
            self.release(&key.session, &lease).await;
        }
        outcome
    }

    async fn settle_leased(
        &self,
        shared: &SharedEntry,
        fence: &Fence<'_>,
        mut committer: Committer,
    ) -> Result<Left, CollabError> {
        let sessions = &self.documents.repositories().edit_sessions;
        let mut settled = Checkpoint::Unchanged;
        for attempt in 0..SETTLE_ATTEMPTS {
            if attempt > 0 {
                committer = self
                    .claim(shared, fence, ClaimWait::No, None)
                    .await?
                    .ok_or_else(|| fence.key.lease_lost())?;
            }
            let checkpoint = self.settle_once(shared, fence, &committer).await?;
            settled = combine(settled, checkpoint);
            let settled_seq = {
                let entry = self.caught_up(shared).await?;
                entry.last_seq
            };
            if sessions
                .close_settled(&fence.key.session, settled_seq, fence.lease)
                .await?
            {
                return Ok(Left {
                    closed: true,
                    checkpoint: settled,
                });
            }
        }
        tracing::warn!(
            session_id = %fence.key.session,
            attempts = SETTLE_ATTEMPTS,
            "Edit session kept receiving updates while it was settled; left open"
        );
        Ok(Left {
            closed: false,
            checkpoint: settled,
        })
    }

    async fn settle_once(
        &self,
        shared: &SharedEntry,
        fence: &Fence<'_>,
        committer: &Committer,
    ) -> Result<Checkpoint, CollabError> {
        match committer {
            Committer::Editor(editor) => match self.checkpoint_leased(shared, editor, fence).await?
            {
                Checkpoint::Conflict { head, .. } => {
                    let rebase = Rebase {
                        head,
                        level: EditLevel::Edit,
                    };
                    self.propose_and_rebase(shared, editor, fence, rebase).await
                },
                other => Ok(other),
            },
            Committer::Commenter(commenter) => {
                self.propose_comment_body(shared, commenter, fence).await
            },
            Committer::Nobody => Ok(Checkpoint::Unchanged),
        }
    }

    pub(super) async fn claim(
        &self,
        shared: &SharedEntry,
        fence: &Fence<'_>,
        wait: ClaimWait,
        known: Option<&AuthenticatedUser>,
    ) -> Result<Option<Committer>, CollabError> {
        let Some(session) = self.claim_row(fence.key, fence.lease, wait).await? else {
            return Ok(None);
        };
        let authors = self
            .documents
            .repositories()
            .collab
            .list_authors_since_checkpoint(&fence.key.session)
            .await?;
        let mut entry = shared.lock().await;
        entry.session = session;
        entry.last_actor = fold_logged(&authors);
        let last_editor = entry.last_editor.clone();
        drop(entry);
        let known: Vec<&AuthenticatedUser> = last_editor.iter().chain(known).collect();
        Ok(Some(Committer::from_log(&authors, &known)))
    }
}

fn combine(settled: Checkpoint, next: Checkpoint) -> Checkpoint {
    match (settled, next) {
        (conflict @ Checkpoint::Conflict { .. }, _) => conflict,
        (settled, Checkpoint::Unchanged) => settled,
        (_, next) => next,
    }
}
