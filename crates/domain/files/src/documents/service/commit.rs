//! The revision engine: [`DocumentService::commit`] executes a
//! [`CommitPlan`].
//!
//! Order: authorize `edit`, refuse when another holder has the lock, load
//! the head, plan, build the new bytes (store, apply ops, or merge three
//! ways), write the blob and the revision, then move the head by
//! compare-and-swap. Index and event failures after the swap are returned as
//! [`CommitWarning`]s; the commit stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentConflictEvent, DocumentFormat, EditOp,
};

use super::support::blocking;
use super::{Author, CommitOutcome, CommitRequest, CommitWarning, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewRevision};
use crate::documents::revision::{CommitBody, CommitPlan, MergeOutcome, three_way_bytes};

enum Built {
    Bytes(Vec<u8>),
    Conflict,
}

struct CommitState<'a> {
    document: &'a Document,
    head: Option<&'a DocumentRevision>,
    base: Option<&'a DocumentRevisionId>,
}

struct Conflicted {
    head: Option<DocumentRevision>,
    base: Option<DocumentRevisionId>,
}

impl DocumentService {
    pub async fn commit(
        &self,
        who: &AuthenticatedUser,
        request: CommitRequest,
        trace: &TraceId,
    ) -> Result<CommitOutcome, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document_id, DocumentAction::Edit, trace)
            .await?;
        self.check_lock(&document, request.lock_holder_id.as_deref())
            .await?;
        let head = self.repos.revisions.head(&document.id).await?;
        let base = request.base_revision.clone();
        let plan = CommitPlan::decide(
            head.as_ref().map(|h| &h.id),
            base.as_ref(),
            document.format,
            &request.body,
        );
        let edit_ops = match &request.body {
            CommitBody::Ops(ops) => Some(ops.clone()),
            CommitBody::Bytes(_) => None,
        };
        let state = CommitState {
            document: &document,
            head: head.as_ref(),
            base: base.as_ref(),
        };
        let bytes = match self.build(plan, &state, request.body).await? {
            Built::Bytes(bytes) => bytes,
            Built::Conflict => {
                return self
                    .conflict(who, &document, Conflicted { head, base }, &request.author)
                    .await;
            },
        };
        self.check_size(bytes.len())?;
        let blob = self.blobs.put(&bytes).await?;
        let parent = head.as_ref().map(|h| h.id.clone());
        let revision = self
            .repos
            .revisions
            .insert(NewRevision {
                document_id: document.id.clone(),
                parent_revision_id: parent.clone(),
                content_sha256: blob.hash,
                author_user_id: who.id.clone(),
                actor_kind: request.author.actor_kind,
                origin_kind: request.author.origin.kind(),
                origin_id: request.author.origin.id(),
                message: request.message,
                edit_ops,
            })
            .await?;
        if !self
            .repos
            .documents
            .set_head_if(&document.id, parent.as_ref(), &revision.id)
            .await?
        {
            let head = self.repos.revisions.head(&document.id).await?;
            return self
                .conflict(who, &document, Conflicted { head, base }, &request.author)
                .await;
        }
        let merged_from = parent.filter(|_| plan.merges());
        Ok(self.finish(&document, revision, bytes, merged_from).await)
    }

    async fn build(
        &self,
        plan: CommitPlan,
        state: &CommitState<'_>,
        body: CommitBody,
    ) -> Result<Built, DocumentsError> {
        let format = state.document.format;
        Ok(match (plan, body) {
            (CommitPlan::NoHead, _) => {
                return Err(DocumentsError::not_found(
                    "head revision of document",
                    &state.document.id,
                ));
            },
            (CommitPlan::Store, CommitBody::Bytes(bytes)) => Built::Bytes(bytes),
            (CommitPlan::Apply, CommitBody::Ops(ops)) => {
                let head_bytes = self.head_bytes(state.head).await?;
                Built::Bytes(self.apply_ops(format, head_bytes, ops).await?)
            },
            (CommitPlan::MergeBytes, CommitBody::Bytes(ours)) => self.merge(state, ours).await?,
            (CommitPlan::MergeOps, CommitBody::Ops(ops)) => {
                let base_bytes = self.base_bytes(state.document, state.base).await?;
                let ours = self.apply_ops(format, base_bytes, ops).await?;
                self.merge(state, ours).await?
            },
            _ => Built::Conflict,
        })
    }

    async fn finish(
        &self,
        document: &Document,
        revision: DocumentRevision,
        bytes: Vec<u8>,
        merged_from: Option<DocumentRevisionId>,
    ) -> CommitOutcome {
        let mut warnings = Vec::new();
        if !self.index_text(document, &revision, bytes).await {
            warnings.push(CommitWarning::SearchIndexStale);
        }
        if !self.announce_revision(document, &revision).await {
            warnings.push(CommitWarning::EventsNotDelivered);
        }
        match merged_from {
            Some(merged_from) => CommitOutcome::Merged {
                revision,
                merged_from,
                warnings,
            },
            None => CommitOutcome::Committed { revision, warnings },
        }
    }

    async fn check_lock(
        &self,
        document: &Document,
        holder_id: Option<&str>,
    ) -> Result<(), DocumentsError> {
        match self.repos.locks.current(&document.id).await? {
            Some(lock) if Some(lock.holder_id.as_str()) != holder_id => {
                Err(DocumentsError::LockHeld {
                    document_id: document.id.clone(),
                    holder_kind: lock.holder_kind,
                    holder_id: lock.holder_id,
                })
            },
            _ => Ok(()),
        }
    }

    async fn head_bytes(&self, head: Option<&DocumentRevision>) -> Result<Vec<u8>, DocumentsError> {
        match head {
            Some(head) => self.revision_bytes(head).await,
            None => Err(DocumentsError::not_found("head revision", "none")),
        }
    }

    async fn base_bytes(
        &self,
        document: &Document,
        base: Option<&DocumentRevisionId>,
    ) -> Result<Vec<u8>, DocumentsError> {
        let base = self.resolve_revision(document, base).await?;
        self.revision_bytes(&base).await
    }

    async fn apply_ops(
        &self,
        format: DocumentFormat,
        bytes: Vec<u8>,
        ops: Vec<EditOp>,
    ) -> Result<Vec<u8>, DocumentsError> {
        let codec = self.codec(format)?;
        blocking(move || {
            let projection = codec.project(&bytes)?;
            Ok(codec.apply(&bytes, &projection, &ops)?)
        })
        .await
    }

    async fn merge(&self, state: &CommitState<'_>, ours: Vec<u8>) -> Result<Built, DocumentsError> {
        let base_bytes = self.base_bytes(state.document, state.base).await?;
        let head_bytes = self.head_bytes(state.head).await?;
        Ok(match three_way_bytes(&base_bytes, &ours, &head_bytes) {
            MergeOutcome::Clean(merged) => Built::Bytes(merged.into_bytes()),
            MergeOutcome::Conflicted => Built::Conflict,
        })
    }

    async fn conflict(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        conflicted: Conflicted,
        author: &Author,
    ) -> Result<CommitOutcome, DocumentsError> {
        let Conflicted { head, base } = conflicted;
        let head = head
            .ok_or_else(|| DocumentsError::not_found("head revision of document", &document.id))?;
        let path = match self.document_path(document).await {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Conflict event carries the bare document name"
                );
                document.name.clone()
            },
        };
        let event = DocumentConflictEvent {
            document_id: document.id.clone(),
            path,
            head_revision_id: head.id.clone(),
            base_revision_id: base.clone(),
            device_id: author.origin.device().cloned(),
        };
        if let Err(error) = self
            .events
            .conflict(std::slice::from_ref(&who.id), &event)
            .await
        {
            tracing::warn!(
                document_id = %document.id,
                error = %error,
                "Conflict event not delivered"
            );
        }
        Ok(CommitOutcome::Conflict { head, base })
    }
}
