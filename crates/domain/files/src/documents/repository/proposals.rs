//! [`DocumentProposalRepository`]: reviewable revisions and their status.
//!
//! A status moves out of `open` exactly once: every transition is an
//! `UPDATE ... WHERE status = 'open'`, so two reviewers racing on one
//! proposal see one winner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, UserId};
use systemprompt_models::documents::ProposalStatus;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentProposal, NewProposal, ProposalReview};

#[derive(Debug, Clone)]
pub struct DocumentProposalRepository {
    pub(super) pools: Pools,
}

struct ProposalRow {
    id: DocumentProposalId,
    document_id: DocumentId,
    base_revision_id: DocumentRevisionId,
    proposed_revision_id: DocumentRevisionId,
    proposer_user_id: UserId,
    actor_kind: String,
    origin_kind: String,
    origin_id: Option<String>,
    status: String,
    message: Option<String>,
    reviewer_user_id: Option<UserId>,
    reviewed_at: Option<DateTime<Utc>>,
    note: Option<String>,
    landed_revision_id: Option<DocumentRevisionId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<ProposalRow> for DocumentProposal {
    type Error = systemprompt_traits::RepositoryError;

    fn try_from(row: ProposalRow) -> RepoResult<Self> {
        Ok(Self {
            actor_kind: parse("document_proposals.actor_kind", &row.actor_kind)?,
            origin_kind: parse("document_proposals.origin_kind", &row.origin_kind)?,
            status: parse("document_proposals.status", &row.status)?,
            id: row.id,
            document_id: row.document_id,
            base_revision_id: row.base_revision_id,
            proposed_revision_id: row.proposed_revision_id,
            proposer_user_id: row.proposer_user_id,
            origin_id: row.origin_id,
            message: row.message,
            reviewer_user_id: row.reviewer_user_id,
            reviewed_at: row.reviewed_at,
            note: row.note,
            landed_revision_id: row.landed_revision_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn convert(rows: Vec<ProposalRow>) -> RepoResult<Vec<DocumentProposal>> {
    rows.into_iter().map(DocumentProposal::try_from).collect()
}

impl DocumentProposalRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewProposal) -> RepoResult<DocumentProposal> {
        sqlx::query_as!(
            ProposalRow,
            r#"INSERT INTO document_proposals
                   (document_id, base_revision_id, proposed_revision_id, proposer_user_id,
                    actor_kind, origin_kind, origin_id, message)
               VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5, $6, $7, $8)
               RETURNING id::text AS "id!: DocumentProposalId",
                         document_id::text AS "document_id!: DocumentId",
                         base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                         proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                         proposer_user_id AS "proposer_user_id: UserId",
                         actor_kind, origin_kind, origin_id, status, message,
                         reviewer_user_id AS "reviewer_user_id: UserId",
                         reviewed_at, note,
                         landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                         created_at, updated_at"#,
            request.document_id.as_str(),
            request.base_revision_id.as_str(),
            request.proposed_revision_id.as_str(),
            request.proposer_user_id.as_str(),
            request.actor_kind.as_str(),
            request.origin_kind.as_str(),
            request.origin_id,
            request.message,
        )
        .fetch_one(&*self.pools.write)
        .await?
        .try_into()
    }

    pub async fn find(&self, id: &DocumentProposalId) -> RepoResult<Option<DocumentProposal>> {
        sqlx::query_as!(
            ProposalRow,
            r#"SELECT id::text AS "id!: DocumentProposalId",
                      document_id::text AS "document_id!: DocumentId",
                      base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                      proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                      proposer_user_id AS "proposer_user_id: UserId",
                      actor_kind, origin_kind, origin_id, status, message,
                      reviewer_user_id AS "reviewer_user_id: UserId",
                      reviewed_at, note,
                      landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                      created_at, updated_at
               FROM document_proposals WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .map(DocumentProposal::try_from)
        .transpose()
    }

    pub async fn list(
        &self,
        document: &DocumentId,
        status: Option<ProposalStatus>,
        limit: i64,
    ) -> RepoResult<Vec<DocumentProposal>> {
        let rows = sqlx::query_as!(
            ProposalRow,
            r#"SELECT id::text AS "id!: DocumentProposalId",
                      document_id::text AS "document_id!: DocumentId",
                      base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                      proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                      proposer_user_id AS "proposer_user_id: UserId",
                      actor_kind, origin_kind, origin_id, status, message,
                      reviewer_user_id AS "reviewer_user_id: UserId",
                      reviewed_at, note,
                      landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                      created_at, updated_at
               FROM document_proposals
               WHERE document_id = $1::text::uuid
                 AND ($2::text IS NULL OR status = $2)
               ORDER BY created_at DESC, id DESC
               LIMIT $3"#,
            document.as_str(),
            status.map(ProposalStatus::as_str),
            limit,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        convert(rows)
    }

    pub async fn open_for_user(
        &self,
        user: &UserId,
        limit: i64,
    ) -> RepoResult<Vec<DocumentProposal>> {
        let rows = sqlx::query_as!(
            ProposalRow,
            r#"SELECT p.id::text AS "id!: DocumentProposalId",
                      p.document_id::text AS "document_id!: DocumentId",
                      p.base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                      p.proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                      p.proposer_user_id AS "proposer_user_id: UserId",
                      p.actor_kind, p.origin_kind, p.origin_id, p.status, p.message,
                      p.reviewer_user_id AS "reviewer_user_id: UserId",
                      p.reviewed_at, p.note,
                      p.landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                      p.created_at, p.updated_at
               FROM document_proposals p
               JOIN documents d ON d.id = p.document_id
               WHERE p.status = 'open' AND d.deleted_at IS NULL
                 AND (d.owner_user_id = $1 OR p.proposer_user_id = $1)
               ORDER BY p.created_at DESC, p.id DESC
               LIMIT $2"#,
            user.as_str(),
            limit,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        convert(rows)
    }

    pub async fn review(
        &self,
        id: &DocumentProposalId,
        review: &ProposalReview,
    ) -> RepoResult<Option<DocumentProposal>> {
        sqlx::query_as!(
            ProposalRow,
            r#"UPDATE document_proposals
               SET status = $2, reviewer_user_id = $3, note = $4, landed_revision_id = $5::text::uuid,
                   reviewed_at = now(), updated_at = now()
               WHERE id = $1::text::uuid AND status = 'open'
               RETURNING id::text AS "id!: DocumentProposalId",
                         document_id::text AS "document_id!: DocumentId",
                         base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                         proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                         proposer_user_id AS "proposer_user_id: UserId",
                         actor_kind, origin_kind, origin_id, status, message,
                         reviewer_user_id AS "reviewer_user_id: UserId",
                         reviewed_at, note,
                         landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                         created_at, updated_at"#,
            id.as_str(),
            review.status.as_str(),
            review.reviewer_user_id.as_ref().map(UserId::as_str),
            review.note,
            review
                .landed_revision_id
                .as_ref()
                .map(DocumentRevisionId::as_str),
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentProposal::try_from)
        .transpose()
    }

    pub async fn close_open(
        &self,
        document: &DocumentId,
        proposer: Option<&UserId>,
        review: &ProposalReview,
    ) -> RepoResult<Vec<DocumentProposal>> {
        let rows = sqlx::query_as!(
            ProposalRow,
            r#"UPDATE document_proposals
               SET status = $3, reviewer_user_id = $4, note = $5,
                   reviewed_at = now(), updated_at = now()
               WHERE document_id = $1::text::uuid AND status = 'open'
                 AND ($2::text IS NULL OR proposer_user_id = $2)
               RETURNING id::text AS "id!: DocumentProposalId",
                         document_id::text AS "document_id!: DocumentId",
                         base_revision_id::text AS "base_revision_id!: DocumentRevisionId",
                         proposed_revision_id::text AS "proposed_revision_id!: DocumentRevisionId",
                         proposer_user_id AS "proposer_user_id: UserId",
                         actor_kind, origin_kind, origin_id, status, message,
                         reviewer_user_id AS "reviewer_user_id: UserId",
                         reviewed_at, note,
                         landed_revision_id::text AS "landed_revision_id: DocumentRevisionId",
                         created_at, updated_at"#,
            document.as_str(),
            proposer.map(UserId::as_str),
            review.status.as_str(),
            review.reviewer_user_id.as_ref().map(UserId::as_str),
            review.note,
        )
        .fetch_all(&*self.pools.write)
        .await?;
        convert(rows)
    }

    pub async fn settle(
        &self,
        id: &DocumentProposalId,
        landed: Option<&DocumentRevisionId>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_proposals
             SET status = CASE WHEN $2::text IS NULL THEN 'open' ELSE status END,
                 reviewer_user_id = CASE WHEN $2::text IS NULL THEN NULL ELSE reviewer_user_id END,
                 reviewed_at = CASE WHEN $2::text IS NULL THEN NULL ELSE reviewed_at END,
                 note = CASE WHEN $2::text IS NULL THEN NULL ELSE note END,
                 landed_revision_id = $2::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid AND status = 'accepted'",
            id.as_str(),
            landed.map(DocumentRevisionId::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
