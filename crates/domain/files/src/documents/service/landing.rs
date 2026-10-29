//! Landing a revision in one transaction on the primary.
//!
//! Inside the transaction: reserve the writer's node id counter, carry ids
//! forward from the parent (on a blocking thread), insert the revision and
//! its ids, claim the op id's acknowledgement, move the head by
//! compare-and-swap (which also refuses a foreign lock), mark the revision
//! landed, and settle the proposal being accepted. Any refusal rolls the
//! whole transaction back, so a commit that loses the race leaves no
//! revision behind. A proposal's revision is stored the same way without
//! moving the head, and stays unlanded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentOpId, DocumentProposalId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::Projection;
use systemprompt_traits::RepositoryError;

use super::support::blocking;
use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{
    AckClaim, Document, DocumentRevision, NewRevision, NodeIdEntry, StoredWriteAck, WriteAck,
};
use crate::documents::nodes::{
    CarryHints, NodeIdMinter, carry_forward, created_nodes, mint_all, with_ids, writer_tag,
    written_nodes,
};
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
    pub(super) lock_holder: Option<&'a str>,
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

struct Carry {
    next: Projection,
    previous: Option<Projection>,
    hints: CarryHints,
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

fn writer_key(revision: &NewRevision) -> String {
    revision
        .attribution
        .session_id
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| revision.origin_id.clone())
        .unwrap_or_else(|| revision.id.to_string())
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
        let mut tx = self.repos.documents.begin().await?;
        let (entries, next, previous) = Self::mint_entries(
            &mut tx,
            &document.id,
            &revision,
            Carry {
                next,
                previous,
                hints,
            },
        )
        .await?;
        let ids = with_ids(next, &entries);
        let detached = if placement.moves_head {
            self.affected_anchors(&document.id, &ids, placement.comments)
                .await?
        } else {
            Vec::new()
        };
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
            if let Some(proposal) = placement.settles {
                self.settle_in(&mut tx, proposal, &revision.id).await?;
            }
        }
        tx.commit().await.map_err(sql)?;
        Ok(LandResult::Landed(Box::new(Stored {
            revision,
            ids,
            ack,
            detached,
        })))
    }

    async fn mint_entries(
        tx: &mut sqlx::PgConnection,
        document: &DocumentId,
        revision: &NewRevision,
        carry: Carry,
    ) -> Result<(Vec<NodeIdEntry>, Projection, Option<Projection>), DocumentsError> {
        let Carry {
            next,
            previous,
            hints,
        } = carry;
        let writer = writer_tag(&writer_key(revision));
        let count = i64::try_from(next.nodes().count()).unwrap_or(i64::MAX);
        let first = DocumentNodeIdRepository::reserve(tx, document, &writer, count).await?;
        let mut minter = NodeIdMinter::new(writer, u64::try_from(first - 1).unwrap_or(0));
        blocking(move || {
            let entries = match previous.as_ref() {
                Some(previous) => carry_forward(previous, &next, &hints, &mut minter),
                None => mint_all(&next, &mut minter),
            };
            Ok((entries, next, previous))
        })
        .await
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
        self.settle_in(&mut tx, proposal, &proposed.id).await?;
        tx.commit().await.map_err(sql)?;
        Ok(true)
    }

    async fn settle_in(
        &self,
        tx: &mut sqlx::PgConnection,
        proposal: &DocumentProposalId,
        revision: &DocumentRevisionId,
    ) -> Result<(), DocumentsError> {
        if DocumentProposalRepository::settle_landed(tx, proposal, revision).await? {
            return Ok(());
        }
        let status = self
            .repos
            .proposals
            .find(proposal)
            .await?
            .map_or(systemprompt_models::documents::ProposalStatus::Open, |p| {
                p.status
            });
        Err(DocumentsError::ProposalClosed {
            proposal_id: proposal.clone(),
            status,
        })
    }

    pub(super) async fn refuse_lock(
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
