//! Following a write that landed on the head outside the live session: an
//! agent over MCP, the API or an import.
//!
//! When the head has moved past the checkpoint base, the replica that claims
//! the checkpoint lease follows it; a replica that cannot claim it leaves the
//! follow to the holder. With nothing of its own in the live model, the head
//! is written back into the live document as the session's next update and
//! recorded as the checkpoint, so an open tab shows the change without anyone
//! typing first. With local edits pending, the checkpoint is made due now, so
//! the timer's own checkpoint (with its back-off) merges the outside write in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::entry::SharedEntry;
use super::lease::{ClaimWait, Fence, SessionKey, SettledBy};
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
        let (key, base, who) = {
            let entry = shared.lock().await;
            let who = entry
                .last_editor
                .clone()
                .or_else(|| entry.last_joiner.clone());
            (
                SessionKey::of(&entry),
                entry.checkpoint_base().cloned(),
                who,
            )
        };
        let (Some(base), Some(who)) = (base, who) else {
            return Ok(false);
        };
        let Some(document) = self
            .documents
            .repositories()
            .documents
            .find(&key.document)
            .await?
        else {
            return Ok(false);
        };
        if document
            .head_revision_id
            .as_ref()
            .is_none_or(|head| *head == base)
        {
            return Ok(false);
        }
        let trace = TraceId::system();
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
        let work = self.follow_leased(shared, &who, &fence, now);
        let outcome = self.fenced(&key, &lease, Box::pin(work)).await;
        self.release(&key.session, &lease).await;
        outcome
    }

    async fn follow_leased(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        fence: &Fence<'_>,
        now: Instant,
    ) -> Result<bool, CollabError> {
        let Some(body) = self.fork_body(shared).await? else {
            return Ok(false);
        };
        let Some(document) = self
            .documents
            .repositories()
            .documents
            .find(&body.key.document)
            .await?
        else {
            return Ok(false);
        };
        let Some(head) = document.head_revision_id else {
            return Ok(false);
        };
        if head == body.base {
            return Ok(false);
        }
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        if self
            .changed_live(body.state.clone(), who, &base_bytes, fence.trace)
            .await
            .is_some()
        {
            let due = now.checked_sub(self.tuning.checkpoint_after).unwrap_or(now);
            shared.lock().await.checkpoint_due_from = Some(due);
            return Ok(true);
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
