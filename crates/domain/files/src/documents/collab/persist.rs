//! Checkpoints of a live edit session; snapshots are the sibling
//! `snapshot` module.
//!
//! A checkpoint materialises the live model and commits it as a revision
//! with origin `edit_session`, based on the session's previous checkpoint
//! (or its base revision); when the revision engine merges it with outside
//! writes, the merged document is re-imported with the live block ids
//! carried over and written back into the live model, logged and broadcast
//! like any other update.
//!
//! An apply validates only the blocks each update touched, so the whole
//! live model is validated before it is checkpointed: a checkpoint of a
//! model that fails the checks is refused, and a settle logs the failure
//! and lands it anyway, so leaving a session never strands its edits.
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

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::committer::{Committer, fold_logged};
use super::entry::SharedEntry;
use super::lease::{ClaimWait, Fence, SessionKey, SettledBy};
use super::persist_commit::Rebase;
use super::{Checkpoint, CollabError, CollabService, Left};
use crate::documents::engine::ModelView;
use crate::documents::model::{EditLevel, Limits, ModelRejection, validate};
use crate::documents::repository::CheckpointLease;

const SETTLE_ATTEMPTS: u32 = 3;

impl CollabService {
    pub(super) async fn checkpoint(
        &self,
        shared: &SharedEntry,
        trace: &TraceId,
    ) -> Result<Option<Checkpoint>, CollabError> {
        let key = SessionKey::of(&*shared.lock().await);
        check_live(shared)
            .await?
            .map_err(CollabError::UpdateRejected)?;
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
        if let Err(rejection) = check_live(shared).await? {
            tracing::warn!(
                session_id = %key.session,
                rejection = %rejection,
                "Live model fails validation; settling it anyway"
            );
        }
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

async fn check_live(shared: &SharedEntry) -> Result<Result<(), ModelRejection>, CollabError> {
    let model = shared.lock().await.doc.export_model()?;
    Ok(match &model {
        ModelView::RichText(model) => validate(model, model, EditLevel::Edit, &Limits::default()),
        ModelView::Body(_) => Ok(()),
        #[cfg(feature = "collab-loro")]
        ModelView::Sheet(_) | ModelView::Deck(_) => Ok(()),
    })
}

fn combine(settled: Checkpoint, next: Checkpoint) -> Checkpoint {
    match (settled, next) {
        (conflict @ Checkpoint::Conflict { .. }, _) => conflict,
        (settled, Checkpoint::Unchanged) => settled,
        (_, next) => next,
    }
}
