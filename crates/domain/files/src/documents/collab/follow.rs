//! Following a write that landed on the head outside the live session: an
//! agent over MCP, the API or an import.
//!
//! A follow runs only while no checkpoint is due or backing off: those
//! checkpoints already merge the outside write in. It decides without the
//! checkpoint lease whether the head moved past the checkpoint base and
//! whether the live model holds edits of its own. With local edits pending,
//! the checkpoint is made due now, so the timer's own checkpoint merges the
//! outside write. With none, the replica that claims the lease re-checks both
//! on a fresh fork and writes the head back into the live document as the
//! session's next update, recorded as the checkpoint, so an open tab shows
//! the change without anyone typing first; a replica that cannot claim it
//! leaves the follow to the holder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::{DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::entry::SharedEntry;
use super::lease::{ClaimWait, Fence, SessionKey, SettledBy};
use super::persist_commit::Body;
use super::reconcile::WriteBack;
use super::{CollabError, CollabService};
use crate::documents::agents::act_as;
use crate::documents::model::EditLevel;
use crate::documents::repository::CheckpointLease;

impl CollabService {
    pub(super) async fn follow_head(
        &self,
        shared: &SharedEntry,
        now: Instant,
    ) -> Result<bool, CollabError> {
        let (key, who) = {
            let entry = shared.lock().await;
            let waiting = entry.checkpoint_due_from.is_some()
                || entry.checkpoint_retry_at.is_some_and(|at| now < at);
            if waiting {
                return Ok(false);
            }
            let who = entry
                .last_editor
                .clone()
                .or_else(|| entry.last_joiner.clone());
            let key = SessionKey::of(&entry);
            drop(entry);
            (key, who)
        };
        let Some(who) = who else {
            return Ok(false);
        };
        let trace = TraceId::system();
        let Some(head) = self.moved_head(shared, &who, &trace).await? else {
            return Ok(false);
        };
        let Followed::Clean = head else {
            let due = now.checked_sub(self.tuning.checkpoint_after).unwrap_or(now);
            shared.lock().await.checkpoint_due_from = Some(due);
            return Ok(true);
        };
        let lease = CheckpointLease::generate();
        let fence = Fence {
            key: &key,
            lease: &lease,
            trace: &trace,
            by: SettledBy::Participant,
        };
        if self
            .claim(shared, &fence, ClaimWait::No, None)
            .await?
            .is_none()
        {
            return Ok(false);
        }
        let work = self.follow_leased(shared, &who, &fence);
        let outcome = self.fenced(&key, &lease, Box::pin(work)).await;
        self.release(&key.session, &lease).await;
        outcome
    }

    async fn moved_head(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<Option<Followed>, CollabError> {
        let Some(body) = self.fork_body(shared).await? else {
            return Ok(None);
        };
        if self.head_past(&body).await?.is_none() {
            return Ok(None);
        }
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        let local = self
            .changed_live(body.state.clone(), who, &base_bytes, trace)
            .await
            .is_some();
        Ok(Some(if local {
            Followed::Edited
        } else {
            Followed::Clean
        }))
    }

    async fn head_past(&self, body: &Body) -> Result<Option<DocumentRevisionId>, CollabError> {
        let head = self
            .documents
            .repositories()
            .documents
            .find(&body.key.document)
            .await?
            .and_then(|document| document.head_revision_id);
        Ok(head.filter(|head| *head != body.base))
    }

    async fn follow_leased(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        fence: &Fence<'_>,
    ) -> Result<bool, CollabError> {
        let Some(body) = self.fork_body(shared).await? else {
            return Ok(false);
        };
        let Some(head) = self.head_past(&body).await? else {
            return Ok(false);
        };
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        if self
            .changed_live(body.state.clone(), who, &base_bytes, fence.trace)
            .await
            .is_some()
        {
            return Ok(false);
        }
        let head_bytes = self.revision_bytes(&head, limit).await?;
        let back = WriteBack {
            who,
            level: EditLevel::Edit,
            merged: &head_bytes,
        };
        let write_back = self.reconcile(shared, &body, back, fence.trace);
        act_as(body.state.actor.clone(), write_back).await?;
        self.record_checkpoint(shared, &body, &head, fence.lease)
            .await?;
        Ok(true)
    }
}

enum Followed {
    Clean,
    Edited,
}
