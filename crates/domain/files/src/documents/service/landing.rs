//! Landing a revision in one transaction on the primary.
//!
//! Before the transaction: carry ids forward from the parent (see
//! `landing_ids`) and check which comment anchors the commit detaches. Inside
//! it: reserve the created nodes' ids and renumber them, insert the revision
//! and its ids, claim the op id's acknowledgement, move the head by
//! compare-and-swap (which also refuses a foreign lock), mark the revision
//! landed, settle the proposal being accepted, and record the
//! `document.revision.landed` brain fact. Any refusal rolls the whole
//! transaction back, so a commit that loses the race leaves no revision behind.
//! A proposal's revision is stored the same way without moving the head, and
//! stays unlanded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentOpId, DocumentProposalId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::Projection;
use systemprompt_traits::RepositoryError;

use super::landing_ids::{carry, reserve};
use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{
    AckClaim, Document, DocumentRevision, LockHolder, NewRevision, NodeIdEntry, StoredWriteAck,
    WriteAck,
};
use crate::documents::nodes::{CarryHints, created_nodes, with_ids, writer_tag, written_nodes};
use crate::documents::repository::{
    DocumentNodeIdRepository, DocumentProposalRepository, DocumentRepository,
    DocumentRevisionRepository, DocumentWriteAckRepository, HeadMove,
};

pub(super) struct Prepared {
    pub(super) revision: NewRevision,
    pub(super) next: Projection,
    pub(super) previous: Option<Projection>,
    pub(super) hints: CarryHints,
    pub(super) merged_from: Option<DocumentRevisionId>,
    pub(super) applied: usize,
}

#[derive(Clone, Copy)]
pub(super) struct OpClaim<'a> {
    pub(super) op_id: &'a DocumentOpId,
    pub(super) user: &'a UserId,
    pub(super) request_sha256: &'a str,
}

#[derive(Clone, Copy)]
pub(super) struct Placement<'a> {
    pub(super) moves_head: bool,
    pub(super) lock_holder: Option<&'a LockHolder>,
    pub(super) op: Option<OpClaim<'a>>,
    pub(super) settles: Option<&'a DocumentProposalId>,
    pub(super) comments: &'a CommentPolicy,
}

impl Placement<'_> {
    pub(super) const fn proposal(comments: &CommentPolicy) -> Placement<'_> {
        Placement {
            moves_head: false,
            lock_holder: None,
            op: None,
            settles: None,
            comments,
        }
    }
}

pub(super) struct Stored {
    pub(super) revision: DocumentRevision,
    pub(super) ids: Projection,
    pub(super) ack: WriteAck,
    pub(super) detached: Vec<DocumentCommentId>,
}

pub(super) enum LandResult {
    Landed(Box<Stored>),
    HeadMoved,
    Claimed(StoredWriteAck),
}

fn sql(error: sqlx::Error) -> DocumentsError {
    DocumentsError::Repository(RepositoryError::from(error))
}

async fn claim_op(
    tx: &mut sqlx::PgConnection,
    document: &DocumentId,
    op: Option<OpClaim<'_>>,
    ack: &WriteAck,
) -> Result<Option<StoredWriteAck>, DocumentsError> {
    let Some(op) = op else {
        return Ok(None);
    };
    let claim = AckClaim {
        document,
        op_id: op.op_id,
        user: op.user,
        request_sha256: op.request_sha256,
        ack,
    };
    Ok(DocumentWriteAckRepository::claim(tx, claim).await?)
}

impl DocumentService {
    pub(super) async fn land(
        &self,
        document: &Document,
        prepared: Prepared,
        placement: Placement<'_>,
    ) -> Result<LandResult, DocumentsError> {
        let Prepared {
            revision,
            next,
            previous,
            hints,
            merged_from,
            applied,
        } = prepared;
        let mut carried = carry(next, previous, hints).await?;
        if let Some(model) = super::live_ids::live_model() {
            carried.pin(&super::live_ids::pinned_ids(&carried.next, &model));
        }
        let detached = if placement.moves_head {
            let entries: Vec<NodeIdEntry> =
                carried.entries.iter().map(|c| c.entry.clone()).collect();
            let provisional = with_ids(carried.next.clone(), &entries);
            self.affected_anchors(&document.id, &provisional, placement.comments)
                .await?
        } else {
            Vec::new()
        };
        let mut tx = self.repos.documents.begin().await?;
        let writer = writer_tag(revision.author_user_id.as_str());
        let entries = reserve(
            &mut tx,
            &document.id,
            &writer,
            carried.entries,
            carried.created,
        )
        .await?;
        let previous = carried.previous;
        let ids = with_ids(carried.next, &entries);
        let parent = revision.parent_revision_id.clone();
        let mut revision = DocumentRevisionRepository::insert_in(&mut tx, revision).await?;
        DocumentNodeIdRepository::insert(&mut tx, &document.id, &revision.id, &entries).await?;
        let ack = WriteAck {
            revision_id: revision.id.clone(),
            parent_revision_id: parent.clone(),
            merged_from,
            applied,
            written: written_nodes(previous.as_ref(), &ids),
            created: created_nodes(previous.as_ref(), &ids),
        };
        if let Some(existing) = claim_op(&mut tx, &document.id, placement.op, &ack).await? {
            return Ok(LandResult::Claimed(existing));
        }
        if placement.moves_head {
            let head = HeadMove {
                document: &document.id,
                expected: parent.as_ref(),
                new: &revision.id,
                lock_holder: placement.lock_holder,
            };
            if !DocumentRepository::move_head(&mut tx, head).await? {
                drop(tx);
                self.refuse_lock(document, placement.lock_holder).await?;
                return Ok(LandResult::HeadMoved);
            }
            revision.landed_at =
                Some(DocumentRevisionRepository::mark_landed(&mut tx, &revision.id).await?);
            if let Some(proposal) = placement.settles
                && !DocumentProposalRepository::settle_landed(&mut tx, proposal, &revision.id)
                    .await?
            {
                drop(tx);
                return Err(self.proposal_closed(proposal).await);
            }
            self.record_revision_landed(&mut tx, document, &revision)
                .await?;
        }
        tx.commit().await.map_err(sql)?;
        Ok(LandResult::Landed(Box::new(Stored {
            revision,
            ids,
            ack,
            detached,
        })))
    }

    pub(super) async fn land_existing(
        &self,
        document: &Document,
        base: &DocumentRevisionId,
        proposed: &mut DocumentRevision,
        proposal: &DocumentProposalId,
    ) -> Result<bool, DocumentsError> {
        let mut tx = self.repos.documents.begin().await?;
        let head = HeadMove {
            document: &document.id,
            expected: Some(base),
            new: &proposed.id,
            lock_holder: None,
        };
        if !DocumentRepository::move_head(&mut tx, head).await? {
            drop(tx);
            self.refuse_lock(document, None).await?;
            return Ok(false);
        }
        proposed.landed_at =
            Some(DocumentRevisionRepository::mark_landed(&mut tx, &proposed.id).await?);
        if !DocumentProposalRepository::settle_landed(&mut tx, proposal, &proposed.id).await? {
            drop(tx);
            return Err(self.proposal_closed(proposal).await);
        }
        self.record_revision_landed(&mut tx, document, proposed)
            .await?;
        tx.commit().await.map_err(sql)?;
        Ok(true)
    }
}
