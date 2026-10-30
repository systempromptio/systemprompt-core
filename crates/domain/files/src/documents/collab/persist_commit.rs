//! Committing a live edit session's checkpoint as a revision, and
//! reconciling a merged or conflicting result back into the live model.
//! The lease, settle loop and invariants are described on the sibling
//! `persist` module.
//!
//! A checkpoint the idle-session reaper settles is recorded with actor kind
//! `system` on behalf of the session's last editor, so its revision says
//! that no participant was present when it landed.
//!
//! The body a checkpoint commits is taken, with the sequence number it
//! covers and a fork of the engine document at that point, while the
//! replica's session entry is held; the commit itself runs without it. The
//! update that writes a merged or head document back is computed on that
//! fork, under the fork's own engine peer, so it applies alongside whatever
//! was edited while the revision landed instead of reverting it.
//!
//! A checkpoint commits under its live model (`with_live_model`), so the
//! new head records each block under the id the session gave it, a block
//! added live included, rather than an id carried or minted afresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Mutex;

use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ActorKind;

use super::entry::{CollabEntry, SharedEntry};
use super::error::model_error;
use super::lease::{Fence, SessionKey, SettledBy};
use super::live_media::{LiveState, materialise_or_strip};
use super::reconcile::WriteBack;
use super::{Checkpoint, CollabError, CollabService};
use crate::documents::agents::act_as;
use crate::documents::engine::{EngineDoc, ModelView};
use crate::documents::model::{EditLevel, Media};
use crate::documents::models::{RevisionAttribution, RevisionOrigin};
use crate::documents::repository::CheckpointLease;
use crate::documents::revision::CommitBody;
use crate::documents::service::{
    Author, CommentPolicy, CommitOutcome, CommitRequest, ProposeBodyRequest, with_live_model,
};

const CHECKPOINT_MESSAGE: &str = "Live edit session checkpoint";
const CONFLICT_MESSAGE: &str = "Live edit session body that conflicted with the head";

pub(super) struct Rebase {
    pub(super) head: DocumentRevisionId,
    pub(super) level: EditLevel,
}

pub(super) struct Body {
    pub(super) key: SessionKey,
    pub(super) base: DocumentRevisionId,
    pub(super) seq: i64,
    pub(super) fork: Mutex<Box<dyn EngineDoc>>,
    pub(super) state: LiveState,
}

impl CollabService {
    pub(super) async fn take_body(
        &self,
        shared: &SharedEntry,
    ) -> Result<Option<Body>, CollabError> {
        let mut entry = self.caught_up(shared).await?;
        let body = body_of_entry(&entry)?;
        if body.is_some() {
            entry.checkpoint_due_from = None;
        }
        drop(entry);
        Ok(body)
    }

    pub(super) async fn fork_body(
        &self,
        shared: &SharedEntry,
    ) -> Result<Option<Body>, CollabError> {
        let entry = self.caught_up(shared).await?;
        body_of_entry(&entry)
    }

    pub(super) async fn checkpoint_leased(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        fence: &Fence<'_>,
    ) -> Result<Checkpoint, CollabError> {
        let trace = fence.trace;
        let Some(body) = self.take_body(shared).await? else {
            return Ok(Checkpoint::Unchanged);
        };
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        let format = body.state.format;
        let Some(live) = self
            .changed_live(body.state.clone(), who, &base_bytes, trace)
            .await
        else {
            return Ok(Checkpoint::Unchanged);
        };
        let next = materialise_or_strip(format, &base_bytes, &live)
            .map_err(model_error(&body.key.document))?;
        let commit = self.documents.commit(
            who,
            CommitRequest {
                document_id: body.key.document.clone(),
                base_revision: Some(body.base.clone()),
                body: CommitBody::Bytes(next),
                author: session_author(&body, who, fence),
                message: Some(CHECKPOINT_MESSAGE.to_owned()),
                lock_holder: None,
                comments: CommentPolicy::detach(),
                op_id: None,
                media: Media::new(),
            },
            trace,
        );
        let commit = act_as(body.state.actor.clone(), commit);
        let outcome = match &live.model {
            ModelView::RichText(model) => Box::pin(with_live_model(model.clone(), commit)).await?,
            _ => Box::pin(commit).await?,
        };
        let revision = match outcome {
            CommitOutcome::Committed(landed) => landed.revision,
            CommitOutcome::Merged(landed) => {
                let revision = landed.revision;
                let merged = self.revision_bytes(&revision.id, limit).await?;
                let back = WriteBack {
                    who,
                    level: EditLevel::Edit,
                    merged: &merged,
                };
                let write_back = self.reconcile(shared, &body, back, trace);
                act_as(body.state.actor.clone(), write_back).await?;
                revision
            },
            CommitOutcome::Conflict { head, .. } => {
                tracing::warn!(
                    document_id = %body.key.document,
                    head_revision_id = %head.id,
                    "Edit session checkpoint conflicted with the head"
                );
                return Ok(Checkpoint::Conflict {
                    head: head.id,
                    proposal: None,
                });
            },
        };
        self.record_checkpoint(shared, &body, &revision.id, fence.lease)
            .await?;
        Ok(Checkpoint::Committed(revision.id))
    }

    pub(super) async fn record_checkpoint(
        &self,
        shared: &SharedEntry,
        body: &Body,
        revision: &DocumentRevisionId,
        lease: &CheckpointLease,
    ) -> Result<(), CollabError> {
        let recorded = self
            .documents
            .repositories()
            .edit_sessions
            .set_checkpoint(&body.key.session, revision, body.seq, lease)
            .await?;
        if !recorded {
            return Err(body.key.lease_lost());
        }
        let mut entry = shared.lock().await;
        entry.session.checkpoint_revision_id = Some(revision.clone());
        entry.last_actor = None;
        drop(entry);
        Ok(())
    }

    pub(super) async fn propose_comment_body(
        &self,
        shared: &SharedEntry,
        commenter: &AuthenticatedUser,
        fence: &Fence<'_>,
    ) -> Result<Checkpoint, CollabError> {
        let document = shared.lock().await.session.document_id.clone();
        let head = self
            .documents
            .repositories()
            .documents
            .find(&document)
            .await?
            .and_then(|found| found.head_revision_id);
        let Some(head) = head else {
            return Ok(Checkpoint::Unchanged);
        };
        let rebase = Rebase {
            head,
            level: EditLevel::Comment,
        };
        self.propose_and_rebase(shared, commenter, fence, rebase)
            .await
    }

    pub(super) async fn propose_and_rebase(
        &self,
        shared: &SharedEntry,
        author: &AuthenticatedUser,
        fence: &Fence<'_>,
        rebase: Rebase,
    ) -> Result<Checkpoint, CollabError> {
        let Rebase { head, level } = rebase;
        let trace = fence.trace;
        let Some(body) = self.take_body(shared).await? else {
            return Ok(Checkpoint::Conflict {
                head,
                proposal: None,
            });
        };
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        let format = body.state.format;
        let Some(live) = self
            .changed_live(body.state.clone(), author, &base_bytes, trace)
            .await
        else {
            return Ok(Checkpoint::Unchanged);
        };
        let bytes = materialise_or_strip(format, &base_bytes, &live)
            .map_err(model_error(&body.key.document))?;
        let proposed = self
            .documents
            .propose_body(
                author,
                ProposeBodyRequest {
                    document_id: body.key.document.clone(),
                    base_revision: body.base.clone(),
                    bytes,
                    author: session_author(&body, author, fence),
                    message: Some(CONFLICT_MESSAGE.to_owned()),
                },
                trace,
            )
            .await?;
        let head_bytes = self.revision_bytes(&head, limit).await?;
        let back = WriteBack {
            who: author,
            level,
            merged: &head_bytes,
        };
        let write_back = self.reconcile(shared, &body, back, trace);
        act_as(body.state.actor.clone(), write_back).await?;
        self.record_checkpoint(shared, &body, &head, fence.lease)
            .await?;
        Ok(Checkpoint::Conflict {
            head,
            proposal: Some(proposed.proposal.id),
        })
    }
}

fn session_author(body: &Body, committer: &AuthenticatedUser, fence: &Fence<'_>) -> Author {
    let (actor_kind, on_behalf_of) = match fence.by {
        SettledBy::Participant => (ActorKind::User, None),
        SettledBy::Reaper => (ActorKind::System, Some(committer.id.clone())),
    };
    Author {
        actor_kind,
        origin: RevisionOrigin::EditSession(body.key.session.clone()),
        attribution: RevisionAttribution {
            trace_id: Some(fence.trace.clone()),
            on_behalf_of,
            ..RevisionAttribution::default()
        },
    }
}

fn body_of_entry(entry: &CollabEntry) -> Result<Option<Body>, CollabError> {
    let Some(base) = entry.checkpoint_base().cloned() else {
        return Ok(None);
    };
    Ok(Some(Body {
        key: SessionKey::of(entry),
        base,
        seq: entry.last_seq,
        fork: Mutex::new(entry.doc.fork()),
        state: LiveState::of(entry)?,
    }))
}
