//! The revision engine: [`DocumentService::commit`] executes a
//! [`CommitPlan`].
//!
//! Order: authorize `edit`, refuse when another holder has the lock, load
//! the head, expand revision ops (`restore`, `insert_from_revision`), plan,
//! build the new bytes (store, apply ops to the head under node guards,
//! merge text three ways, or diff an Office upload into ops on the head),
//! write the blob and the revision, record the revision's stable node ids,
//! then move the head by compare-and-swap. Index and event failures after
//! the swap are returned as [`CommitWarning`]s; the commit stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, EditOp};

use super::{CommitOutcome, CommitRequest, CommitWarning, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewRevision};
use crate::documents::revision::{CommitBody, CommitPlan};

pub(super) enum Built {
    Bytes(Vec<u8>),
    Conflict,
}

pub(super) struct Landing<'a> {
    pub document: &'a Document,
    pub head: Option<&'a DocumentRevision>,
    pub base: Option<&'a DocumentRevisionId>,
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
        let landing = Landing {
            document: &document,
            head: head.as_ref(),
            base: base.as_ref(),
        };
        let Some(body) = self.expand(&landing, request.body).await? else {
            return self
                .conflict(who, &document, head, base, &request.author)
                .await;
        };
        let head_id = head.as_ref().map(|h| &h.id);
        let plan = CommitPlan::decide(head_id, base.as_ref(), document.format, &body);
        let merged = plan.merges(head_id, base.as_ref());
        let edit_ops = match &body {
            CommitBody::Ops(ops) => Some(ops.clone()),
            CommitBody::Bytes(_) => None,
        };
        let (built, previous) = self.build(plan, &landing, body).await?;
        let Built::Bytes(bytes) = built else {
            return self
                .conflict(who, &document, head, base, &request.author)
                .await;
        };
        self.check_size(bytes.len())?;
        let blob = self.blobs.put(&bytes).await?;
        let parent = head.as_ref().map(|h| h.id.clone());
        let author = request.author;
        let revision = self
            .repos
            .revisions
            .insert(NewRevision {
                document_id: document.id.clone(),
                parent_revision_id: parent.clone(),
                content_sha256: blob.hash,
                author_user_id: who.id.clone(),
                actor_kind: author.actor_kind,
                origin_kind: author.origin.kind(),
                origin_id: author.origin.id(),
                message: request.message,
                edit_ops,
                attribution: author.attribution.clone(),
            })
            .await?;
        let next = self.project_paths(document.format, bytes.clone()).await?;
        self.record_ids(&document, &revision, previous.as_ref(), &next)
            .await?;
        if !self
            .repos
            .documents
            .set_head_if(&document.id, parent.as_ref(), &revision.id)
            .await?
        {
            let head = self.repos.revisions.head(&document.id).await?;
            return self.conflict(who, &document, head, base, &author).await;
        }
        let merged_from = parent.filter(|_| merged);
        Ok(self.finish(&document, revision, bytes, merged_from).await)
    }

    async fn expand(
        &self,
        landing: &Landing<'_>,
        body: CommitBody,
    ) -> Result<Option<CommitBody>, DocumentsError> {
        let CommitBody::Ops(ops) = body else {
            return Ok(Some(body));
        };
        if let [EditOp::Restore { revision_id }] = ops.as_slice() {
            let at_head = landing
                .head
                .is_some_and(|head| landing.base == Some(&head.id));
            if !at_head {
                return Ok(None);
            }
            let restored = self
                .resolve_revision(landing.document, Some(revision_id))
                .await?;
            return Ok(Some(CommitBody::Bytes(
                self.revision_bytes(&restored).await?,
            )));
        }
        let mut expanded = Vec::with_capacity(ops.len());
        for op in ops {
            expanded.push(self.expand_op(landing.document, op).await?);
        }
        Ok(Some(CommitBody::Ops(expanded)))
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
}
