//! Moving a document's head: the compare-and-swap that lands a commit, run
//! inside the commit transaction.
//!
//! The swap succeeds only when the head is still the revision the commit was
//! built on, the document is not trashed, the new revision belongs to it, and
//! no unexpired lock is held by anyone but the committing holder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, Postgres, Transaction};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};

use super::documents::DocumentRepository;
use super::rows::RepoResult;

#[derive(Debug, Clone, Copy)]
pub struct HeadMove<'a> {
    pub document: &'a DocumentId,
    pub expected: Option<&'a DocumentRevisionId>,
    pub new: &'a DocumentRevisionId,
    pub lock_holder: Option<&'a str>,
}

impl DocumentRepository {
    pub async fn begin(&self) -> RepoResult<Transaction<'static, Postgres>> {
        Ok(self.pools.write.begin().await?)
    }

    pub async fn move_head(conn: &mut PgConnection, head: HeadMove<'_>) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET head_revision_id = $3::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid
               AND head_revision_id IS NOT DISTINCT FROM $2::text::uuid
               AND deleted_at IS NULL
               AND EXISTS (SELECT 1 FROM document_revisions r
                           WHERE r.id = $3::text::uuid AND r.document_id = $1::text::uuid)
               AND NOT EXISTS (SELECT 1 FROM document_locks l
                               WHERE l.document_id = $1::text::uuid
                                 AND l.expires_at > now()
                                 AND l.holder_id IS DISTINCT FROM $4)",
            head.document.as_str(),
            head.expected.map(DocumentRevisionId::as_str),
            head.new.as_str(),
            head.lock_holder,
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
