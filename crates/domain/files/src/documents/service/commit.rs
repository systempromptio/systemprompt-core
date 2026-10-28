//! The revision engine: [`DocumentService::commit`] executes a
//! [`CommitPlan`].
//!
//! Order: authorize `edit`, check the input limits, answer a repeated op id
//! from its stored acknowledgement (also after a refusal, when a concurrent
//! call with the same op id landed first), refuse when another holder has the
//! lock, load the head from the primary, refuse a whole-document op
//! (`set_text`, `set_html`, `restore`) whose base is not the head, since it
//! touches every node, expand revision ops (`restore`, `insert_from_revision`),
//! plan, build the new bytes (store, apply ops to the head under node guards,
//! merge text three ways, or diff an Office upload into ops on the head),
//! write the blob, and land the revision in one transaction (see
//! `landing`): ids, op id acknowledgement and the head's compare-and-swap
//! commit together or not at all. Comments the commit detached or answered
//! are settled after it lands. Index and event failures after the swap are
//! returned as [`CommitWarning`]s; the commit stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentProposalId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, EditOp};
use systemprompt_provider_contracts::documents::NodeFragment;

use super::agent_rules::body_ops;
use super::apply::AppliedOps;
use super::commit_build::Conflicted;
use super::landed::Pending;
use super::landing::{OpClaim, Placement};
use super::{CommitOutcome, CommitRequest, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::nodes::CarryHints;
use crate::documents::revision::CommitBody;

pub(super) enum Built {
    Applied(AppliedOps),
    Conflict,
}

impl Built {
    pub(super) fn stored(bytes: Vec<u8>) -> Self {
        Self::Applied(AppliedOps {
            bytes,
            hints: CarryHints::default(),
        })
    }
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    document: &'a Document,
    settles: Option<&'a DocumentProposalId>,
    digest: &'a str,
}


pub(super) struct Landing<'a> {
    pub document: &'a Document,
    pub head: Option<&'a DocumentRevision>,
    pub base: Option<&'a DocumentRevisionId>,
    pub media: &'a Media,
}

impl DocumentService {
    pub async fn commit(
        &self,
        who: &AuthenticatedUser,
        request: CommitRequest,
        trace: &TraceId,
    ) -> Result<CommitOutcome, DocumentsError> {
        self.commit_settling(who, request, None, trace).await
    }

    pub(super) async fn commit_settling(
        &self,
        who: &AuthenticatedUser,
        request: CommitRequest,
        settles: Option<&DocumentProposalId>,
        trace: &TraceId,
    ) -> Result<CommitOutcome, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document_id, DocumentAction::Edit, trace)
            .await?;
        self.check_body_limits(&request.body)?;
        self.agent_write_rules(who, &document, &request.body)
            .await?;
        let digest = super::acks::request_digest(request.base_revision.as_ref(), &request.body)?;
        let scope = Scope {
            document: &document,
            settles,
            digest: &digest,
        };
        let ops = body_ops(&request.body);
        let Some(op_id) = request.op_id.clone() else {
            self.check_agent_edits(who, ops)?;
            let outcome = self.commit_document(who, request, scope).await;
            self.record_landed(who, ops, &outcome);
            return outcome;
        };
        if let Some(stored) = self.repos.write_acks.find(&document.id, &op_id).await? {
            return self.replay(who, &op_id, &digest, stored).await;
        }
        self.check_agent_edits(who, ops)?;
        let outcome = self.commit_document(who, request, scope).await;
        self.record_landed(who, ops, &outcome);
        if matches!(outcome, Err(_) | Ok(CommitOutcome::Conflict { .. }))
            && let Some(stored) = self.repos.write_acks.find(&document.id, &op_id).await?
        {
            return self.replay(who, &op_id, &digest, stored).await;
        }
        outcome
    }

    async fn commit_document(
        &self,
        who: &AuthenticatedUser,
        request: CommitRequest,
        Scope {
            document,
            settles,
            digest,
        }: Scope<'_>,
    ) -> Result<CommitOutcome, DocumentsError> {
        let document = document.clone();
        self.refuse_lock(&document, request.lock_holder_id.as_deref())
            .await?;
        let head = self.repos.revisions.head(&document.id).await?;
        let base = request.base_revision.clone();
        let landing = Landing {
            document: &document,
            head: head.as_ref(),
            base: base.as_ref(),
            media: &request.media,
        };
        let Some(applied) = self.build_body(&landing, request.body).await? else {
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
        let bytes = applied.ops.bytes.clone();
        let prepared = self
            .prepare(
                who,
                &document,
                super::prepare::Draft {
                    applied,
                    parent,
                    message: request.message,
                    author: &request.author,
                },
            )
            .await?;
        let placement = Placement {
            moves_head: true,
            lock_holder: request.lock_holder_id.as_deref(),
            op: request.op_id.as_ref().map(|op_id| OpClaim {
                op_id,
                user: &who.id,
                request_sha256: digest,
            }),
            settles,
            comments: &request.comments,
        };
        let result = self.land(&document, prepared, placement).await?;
        self.after_landing(
            who,
            &document,
            result,
            Pending {
                op_id: request.op_id.as_ref(),
                digest,
                comments: &request.comments,
                base,
                author: &request.author,
                bytes,
            },
        )
        .await
    }

    async fn build_body(
        &self,
        landing: &Landing<'_>,
        body: CommitBody,
    ) -> Result<Option<super::prepare::Applied>, DocumentsError> {
        match self.expand(landing, body).await? {
            Some((body, fragments)) => self.apply_body(landing, body, fragments).await,
            None => Ok(None),
        }
    }

    async fn expand(
        &self,
        landing: &Landing<'_>,
        body: CommitBody,
    ) -> Result<Option<(CommitBody, Vec<NodeFragment>)>, DocumentsError> {
        let CommitBody::Ops(ops) = body else {
            return Ok(Some((body, Vec::new())));
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
            return Ok(Some((
                CommitBody::Bytes(self.parent_bytes(&restored).await?),
                Vec::new(),
            )));
        }
        let mut expanded = Vec::with_capacity(ops.len());
        let mut fragments = Vec::new();
        for op in ops {
            let (op, fragment) = self.expand_op(landing.document, op).await?;
            expanded.push(op);
            fragments.extend(fragment);
        }
        Ok(Some((CommitBody::Ops(expanded), fragments)))
    }
}
