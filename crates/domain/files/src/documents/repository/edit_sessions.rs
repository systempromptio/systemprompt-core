//! [`DocumentEditSessionRepository`]: at most one open edit session per
//! document.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ContentHash, DocumentEditSessionId, DocumentId, DocumentRevisionId,
};
use systemprompt_traits::{ConstraintKind, RepositoryError};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{DocumentEditSession, EditSessionOpen};

const OPEN_SESSION_INDEX: &str = "uq_document_edit_sessions_open";

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
        .fetch_optional(&*self.pools.read)
        .await?)
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

    pub async fn set_checkpoint(
        &self,
        id: &DocumentEditSessionId,
        revision: &DocumentRevisionId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_edit_sessions SET checkpoint_revision_id = $2::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL",
            id.as_str(),
            revision.as_str(),
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
