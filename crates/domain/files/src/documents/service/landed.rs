//! After the landing transaction: settling the comments a commit detached
//! or answered, indexing and announcing the new revision, and shaping the
//! outcome (a replay, or a conflict when the head moved first).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentOpId, DocumentProposalId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ProposalStatus;

use super::commit_build::Conflicted;
use super::landing::{LandResult, Stored};
use super::{Author, CommentPolicy, CommitOutcome, CommitWarning, DocumentService, LandedCommit};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, LockHolder};

pub(super) struct Pending<'a> {
    pub(super) op_id: Option<&'a DocumentOpId>,
    pub(super) digest: &'a str,
    pub(super) comments: &'a CommentPolicy,
    pub(super) base: Option<DocumentRevisionId>,
    pub(super) author: &'a Author,
    pub(super) bytes: Vec<u8>,
}

impl DocumentService {
    pub(super) async fn after_landing(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        result: LandResult,
        pending: Pending<'_>,
    ) -> Result<CommitOutcome, DocumentsError> {
        match result {
            LandResult::Landed(stored) => {
                self.settle_comments(
                    &document.id,
                    &stored.revision.id,
                    &stored.detached,
                    pending.comments,
                )
                .await?;
                Ok(self.finish(document, *stored, pending.bytes).await)
            },
            LandResult::Claimed(existing) => match pending.op_id {
                Some(op_id) => self.replay(who, op_id, pending.digest, existing).await,
                None => Err(DocumentsError::not_found(
                    "write acknowledgement",
                    &document.id,
                )),
            },
            LandResult::HeadMoved => {
                let head = self.repos.revisions.head(&document.id).await?;
                self.conflict(
                    who,
                    document,
                    Conflicted {
                        head,
                        base: pending.base,
                        author: pending.author,
                    },
                )
                .await
            },
        }
    }

    pub(super) async fn announce_landed(
        &self,
        document: &Document,
        revision: &DocumentRevision,
        bytes: Vec<u8>,
    ) -> Vec<CommitWarning> {
        let mut warnings = Vec::new();
        if !self.index_text(document, revision, bytes).await {
            warnings.push(CommitWarning::SearchIndexStale);
        }
        if !self.announce_revision(document, revision).await {
            warnings.push(CommitWarning::EventsNotDelivered);
        }
        warnings
    }

    async fn finish(&self, document: &Document, stored: Stored, bytes: Vec<u8>) -> CommitOutcome {
        let warnings = self
            .announce_landed(document, &stored.revision, bytes)
            .await;
        let merged = stored.ack.merged_from.is_some();
        let landed = Box::new(LandedCommit {
            revision: stored.revision,
            ack: stored.ack,
            projection: stored.ids,
            replayed: false,
            warnings,
        });
        if merged {
            CommitOutcome::Merged(landed)
        } else {
            CommitOutcome::Committed(landed)
        }
    }

    pub(super) async fn proposal_closed(&self, proposal: &DocumentProposalId) -> DocumentsError {
        match self.repos.proposals.find(proposal).await {
            Ok(found) => DocumentsError::ProposalClosed {
                proposal_id: proposal.clone(),
                status: found.map_or(ProposalStatus::Open, |p| p.status),
            },
            Err(error) => error.into(),
        }
    }

    pub(super) async fn refuse_lock(
        &self,
        document: &Document,
        holder: Option<&LockHolder>,
    ) -> Result<(), DocumentsError> {
        match self.repos.locks.current(&document.id).await? {
            Some(lock) if Some(&lock.holder) != holder => Err(DocumentsError::LockHeld {
                document_id: document.id.clone(),
                holder: lock.holder,
            }),
            _ => Ok(()),
        }
    }
}
