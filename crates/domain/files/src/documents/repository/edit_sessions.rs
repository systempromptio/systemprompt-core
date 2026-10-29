//! [`DocumentEditSessionRepository`]: at most one open edit session per
//! document.
//!
//! A checkpoint of a session runs under a [`CheckpointLease`]:
//! [`DocumentEditSessionRepository::claim_checkpoint`] takes the session row
//! under its row lock, records the lease when no unexpired one is held and
//! returns the row as it stands, so one replica at a time checkpoints a
//! session from its current base. A checkpoint that outlives the hold renews
//! it ([`DocumentEditSessionRepository::renew_checkpoint`]) before it lapses.
//! Moving the checkpoint, which records the sequence number the checkpoint
//! covers, and closing the session both require the lease, and closing also
//! requires that no update was logged after the sequence number the settled
//! body covers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ContentHash, DocumentEditSessionId, DocumentId, DocumentRevisionId,
};
use systemprompt_traits::{ConstraintKind, RepositoryError};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{DocumentEditSession, EditSessionOpen};

const OPEN_SESSION_INDEX: &str = "uq_document_edit_sessions_open";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckpointLease(uuid::Uuid);

impl CheckpointLease {
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    #[must_use]
    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointClaim {
    Claimed(DocumentEditSession),
    Busy,
    Closed,
}

#[derive(Debug, Clone)]
pub struct DocumentEditSessionRepository {
    pools: Pools,
}

impl DocumentEditSessionRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn open(
        &self,
        document_id: &DocumentId,
        base_revision: Option<&DocumentRevisionId>,
    ) -> RepoResult<EditSessionOpen> {
        let result = sqlx::query_as!(
            DocumentEditSession,
            r#"INSERT INTO document_edit_sessions (document_id, base_revision_id)
               VALUES ($1::text::uuid, $2::text::uuid)
               RETURNING id::text AS "id!: DocumentEditSessionId",
                      document_id::text AS "document_id!: DocumentId",
                      base_revision_id::text AS "base_revision_id: DocumentRevisionId",
                      snapshot_sha256 AS "snapshot_sha256: ContentHash",
                      checkpoint_revision_id::text AS "checkpoint_revision_id: DocumentRevisionId",
                      opened_at, updated_at, closed_at"#,
            document_id.as_str(),
            base_revision.map(DocumentRevisionId::as_str),
        )
        .fetch_one(&*self.pools.write)
        .await
        .map_err(RepositoryError::from);
        match result {
            Ok(session) => Ok(EditSessionOpen::Opened(session)),
            Err(RepositoryError::Constraint {
                kind: ConstraintKind::Unique,
                constraint,
                ..
            }) if constraint == OPEN_SESSION_INDEX => Ok(EditSessionOpen::AlreadyOpen),
            Err(other) => Err(other),
        }
    }

    pub async fn find_open(
        &self,
        document_id: &DocumentId,
    ) -> RepoResult<Option<DocumentEditSession>> {
        Ok(sqlx::query_as!(
            DocumentEditSession,
            r#"SELECT id::text AS "id!: DocumentEditSessionId",
                      document_id::text AS "document_id!: DocumentId",
                      base_revision_id::text AS "base_revision_id: DocumentRevisionId",
                      snapshot_sha256 AS "snapshot_sha256: ContentHash",
                      checkpoint_revision_id::text AS "checkpoint_revision_id: DocumentRevisionId",
                      opened_at, updated_at, closed_at
               FROM document_edit_sessions
               WHERE document_id = $1::text::uuid AND closed_at IS NULL"#,
            document_id.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn list_stale(
        &self,
        idle: Duration,
        limit: i64,
    ) -> RepoResult<Vec<DocumentEditSession>> {
        Ok(sqlx::query_as!(
            DocumentEditSession,
            r#"SELECT s.id::text AS "id!: DocumentEditSessionId",
                      s.document_id::text AS "document_id!: DocumentId",
                      s.base_revision_id::text AS "base_revision_id: DocumentRevisionId",
                      s.snapshot_sha256 AS "snapshot_sha256: ContentHash",
                      s.checkpoint_revision_id::text AS "checkpoint_revision_id: DocumentRevisionId",
                      s.opened_at, s.updated_at, s.closed_at
               FROM document_edit_sessions s
               WHERE s.closed_at IS NULL
                 AND s.updated_at < now() - make_interval(secs => $1)
                 AND (s.checkpoint_lease_until IS NULL OR s.checkpoint_lease_until < now())
                 AND NOT EXISTS (
                     SELECT 1 FROM document_presence p
                     WHERE p.document_id = s.document_id AND p.source = 'web'
                       AND p.expires_at > now())
               ORDER BY s.updated_at
               LIMIT $2"#,
            idle.as_secs_f64(),
            limit.max(1),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn claim_checkpoint(
        &self,
        id: &DocumentEditSessionId,
        lease: &CheckpointLease,
        hold: Duration,
    ) -> RepoResult<CheckpointClaim> {
        let claimed = sqlx::query_as!(
            DocumentEditSession,
            r#"UPDATE document_edit_sessions
               SET checkpoint_lease_id = $2,
                   checkpoint_lease_until = now() + make_interval(secs => $3)
               WHERE id = $1::text::uuid AND closed_at IS NULL
                 AND (checkpoint_lease_until IS NULL OR checkpoint_lease_until < now()
                      OR checkpoint_lease_id = $2)
               RETURNING id::text AS "id!: DocumentEditSessionId",
                      document_id::text AS "document_id!: DocumentId",
                      base_revision_id::text AS "base_revision_id: DocumentRevisionId",
                      snapshot_sha256 AS "snapshot_sha256: ContentHash",
                      checkpoint_revision_id::text AS "checkpoint_revision_id: DocumentRevisionId",
                      opened_at, updated_at, closed_at"#,
            id.as_str(),
            lease.as_uuid(),
            hold.as_secs_f64(),
        )
        .fetch_optional(&*self.pools.write)
        .await?;
        if let Some(session) = claimed {
            return Ok(CheckpointClaim::Claimed(session));
        }
        let open = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM document_edit_sessions
                   WHERE id = $1::text::uuid AND closed_at IS NULL) AS "open!""#,
            id.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        Ok(if open {
            CheckpointClaim::Busy
        } else {
            CheckpointClaim::Closed
        })
    }

    pub async fn release_checkpoint(
        &self,
        id: &DocumentEditSessionId,
        lease: &CheckpointLease,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions
             SET checkpoint_lease_id = NULL, checkpoint_lease_until = NULL
             WHERE id = $1::text::uuid AND checkpoint_lease_id = $2",
            id.as_str(),
            lease.as_uuid(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_snapshot(
        &self,
        id: &DocumentEditSessionId,
        hash: &ContentHash,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions SET snapshot_sha256 = $2, updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL",
            id.as_str(),
            hash.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn renew_checkpoint(
        &self,
        id: &DocumentEditSessionId,
        lease: &CheckpointLease,
        hold: Duration,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions
             SET checkpoint_lease_until = now() + make_interval(secs => $3)
             WHERE id = $1::text::uuid AND closed_at IS NULL AND checkpoint_lease_id = $2
               AND checkpoint_lease_until > now()",
            id.as_str(),
            lease.as_uuid(),
            hold.as_secs_f64(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_checkpoint(
        &self,
        id: &DocumentEditSessionId,
        revision: &DocumentRevisionId,
        covered_seq: i64,
        lease: &CheckpointLease,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions
             SET checkpoint_revision_id = $2::text::uuid,
                 checkpoint_seq = GREATEST(checkpoint_seq, $4),
                 updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL AND checkpoint_lease_id = $3",
            id.as_str(),
            revision.as_str(),
            lease.as_uuid(),
            covered_seq,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn close_settled(
        &self,
        id: &DocumentEditSessionId,
        settled_seq: i64,
        lease: &CheckpointLease,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions
             SET closed_at = now(), updated_at = now(),
                 checkpoint_lease_id = NULL, checkpoint_lease_until = NULL
             WHERE id = $1::text::uuid AND closed_at IS NULL
               AND next_seq = $2::bigint + 1 AND checkpoint_lease_id = $3",
            id.as_str(),
            settled_seq,
            lease.as_uuid(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn close(&self, id: &DocumentEditSessionId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions SET closed_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL",
            id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
