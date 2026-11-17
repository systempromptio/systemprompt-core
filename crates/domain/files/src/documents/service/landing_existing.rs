//! Landing a proposal's stored revision: the head moves to it by
//! compare-and-swap, the revision is marked landed, the proposal settles and
//! the `document.revision.landed` brain fact is recorded, in one
//! transaction that runs again when Postgres aborts it as a deadlock victim.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentProposalId, DocumentRevisionId};

use super::DocumentService;
use super::landing::sql;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::repository::{
    DocumentProposalRepository, DocumentRepository, DocumentRevisionRepository, HeadMove,
};
use crate::documents::transient::retry_conflicts;

impl DocumentService {
    pub(super) async fn land_existing(
        &self,
        document: &Document,
        base: &DocumentRevisionId,
        proposed: &mut DocumentRevision,
        proposal: &DocumentProposalId,
    ) -> Result<bool, DocumentsError> {
        let proposed_now = &*proposed;
        let landed_at =
            retry_conflicts(|| self.land_existing_once(document, base, proposed_now, proposal))
                .await?;
        proposed.landed_at = landed_at;
        Ok(landed_at.is_some())
    }

    async fn land_existing_once(
        &self,
        document: &Document,
        base: &DocumentRevisionId,
        proposed: &DocumentRevision,
        proposal: &DocumentProposalId,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, DocumentsError> {
        let mut proposed = proposed.clone();
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
            return Ok(None);
        }
        proposed.landed_at =
            Some(DocumentRevisionRepository::mark_landed(&mut tx, &proposed.id).await?);
        if !DocumentProposalRepository::settle_landed(&mut tx, proposal, &proposed.id).await? {
            drop(tx);
            return Err(self.proposal_closed(proposal).await);
        }
        self.record_revision_landed(&mut tx, document, &proposed)
            .await?;
        tx.commit().await.map_err(sql)?;
        Ok(proposed.landed_at)
    }
}
