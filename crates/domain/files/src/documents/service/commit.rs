//! The revision engine: [`DocumentService::commit`] executes a
//! [`CommitPlan`].
//!
//! Order: authorize `edit`, refuse when another holder has the lock, load
//! the head, refuse a whole-document op (`set_text`, `set_html`, `restore`)
//! whose base is not the head, since it touches every node, expand revision
//! ops (`restore`, `insert_from_revision`), plan,
//! build the new bytes (store, apply ops to the head under node guards,
//! merge text three ways, or diff an Office upload into ops on the head),
//! write the blob and the revision, record the revision's stable node ids,
//! refuse when the new revision drops text a comment is anchored to (unless
//! the request allows detaching), then move the head by compare-and-swap and
//! settle the comments the commit detached or answered. Index and event failures after
//! the swap are returned as [`CommitWarning`]s; the commit stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, EditOp, Projection};

use super::apply::AppliedOps;
use super::commit_build::Conflicted;
use super::node_ids::IdSources;
use super::{Author, CommitOutcome, CommitRequest, CommitWarning, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NewRevision};
use crate::documents::revision::{CommitBody, CommitPlan};

pub(super) enum Built {
    Applied(AppliedOps),
    Conflict,
}

impl Built {
    pub(super) fn stored(bytes: Vec<u8>) -> Self {
        Self::Applied(AppliedOps {
            bytes,
            retired: HashSet::new(),
        })
    }
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
        let applied = match self.expand(&landing, request.body).await? {
            Some(body) => self.apply_body(&landing, body).await?,
            None => None,
        };
        let Some((AppliedOps { bytes, retired }, previous, merged, edit_ops)) = applied else {
            return self
                .conflict(
                    who,
                    &document,
                    Conflicted {
                        head,
                        base,
                        author: &request.author,
                    },
                )
                .await;
        };
        let parent = head.as_ref().map(|h| h.id.clone());
        let author = request.author;
        let (revision, ids) = self
            .store_revision(
                who,
                &document,
                Landed {
                    bytes: &bytes,
                    retired,
                    previous,
                    parent: parent.clone(),
                    edit_ops,
                    message: request.message,
                    author: &author,
                },
            )
            .await?;
        let detached = self
            .affected_anchors(&document.id, &ids, &request.comments)
            .await?;
        if !self
            .repos
            .documents
            .set_head_if(&document.id, parent.as_ref(), &revision.id)
            .await?
        {
            let head = self.repos.revisions.head(&document.id).await?;
            return self
                .conflict(
                    who,
                    &document,
                    Conflicted {
                        head,
                        base,
                        author: &author,
                    },
                )
                .await;
        }
        self.settle_comments(&document.id, &revision.id, &detached, &request.comments)
            .await?;
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
        let at_head = landing
            .head
            .is_some_and(|head| landing.base == Some(&head.id));
        let whole_document = ops.iter().any(|op| {
            matches!(
                op,
                EditOp::SetText { .. } | EditOp::SetHtml { .. } | EditOp::Restore { .. }
            )
        });
        if whole_document && landing.head.is_some() && !at_head {
            return Ok(None);
        }
        if let [EditOp::Restore { revision_id }] = ops.as_slice() {
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

struct Landed<'a> {
    bytes: &'a [u8],
    retired: HashSet<DocumentNodeId>,
    previous: Option<Projection>,
    parent: Option<DocumentRevisionId>,
    edit_ops: Option<Vec<EditOp>>,
    message: Option<String>,
    author: &'a Author,
}

type Applied = (AppliedOps, Option<Projection>, bool, Option<Vec<EditOp>>);

impl DocumentService {
    async fn apply_body(
        &self,
        landing: &Landing<'_>,
        body: CommitBody,
    ) -> Result<Option<Applied>, DocumentsError> {
        let head_id = landing.head.map(|h| &h.id);
        let plan = CommitPlan::decide(head_id, landing.base, landing.document.format, &body);
        let merged = plan.merges(head_id, landing.base);
        let edit_ops = match &body {
            CommitBody::Ops(ops) => Some(ops.clone()),
            CommitBody::Bytes(_) => None,
        };
        let (built, previous) = self.build(plan, landing, body).await?;
        Ok(match built {
            Built::Applied(applied) => Some((applied, previous, merged, edit_ops)),
            Built::Conflict => None,
        })
    }

    async fn store_revision(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        landed: Landed<'_>,
    ) -> Result<(DocumentRevision, Projection), DocumentsError> {
        self.check_size(landed.bytes.len())?;
        let blob = self.blobs.put(landed.bytes).await?;
        let author = landed.author;
        let revision = self
            .repos
            .revisions
            .insert(NewRevision {
                document_id: document.id.clone(),
                parent_revision_id: landed.parent,
                content_sha256: blob.hash,
                author_user_id: who.id.clone(),
                actor_kind: author.actor_kind,
                origin_kind: author.origin.kind(),
                origin_id: author.origin.id(),
                message: landed.message,
                edit_ops: landed.edit_ops,
                attribution: author.attribution.clone(),
            })
            .await?;
        let next = self
            .project_paths(document.format, landed.bytes.to_vec())
            .await?;
        let ids = self
            .record_ids(
                document,
                &revision,
                IdSources {
                    previous: landed.previous.as_ref(),
                    next: &next,
                    retired: &landed.retired,
                },
            )
            .await?;
        Ok((revision, ids))
    }
}
