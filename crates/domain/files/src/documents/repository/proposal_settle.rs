//! Settling an accepted proposal inside the transaction that lands it, and
//! reopening proposals whose landing never completed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{DocumentProposalId, DocumentRevisionId};

use super::proposals::DocumentProposalRepository;
use super::rows::RepoResult;

impl DocumentProposalRepository {
    pub async fn settle_landed(
        conn: &mut PgConnection,
        id: &DocumentProposalId,
        landed: &DocumentRevisionId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_proposals
             SET landed_revision_id = $2::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid AND status = 'accepted' AND landed_revision_id IS NULL",
            id.as_str(),
            landed.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn reopen_unlanded(&self, older_than_minutes: i32, limit: i64) -> RepoResult<u64> {
        let result = sqlx::query!(
            "UPDATE document_proposals
             SET status = 'open', reviewer_user_id = NULL, reviewed_at = NULL, note = NULL,
                 updated_at = now()
             WHERE id IN (
                 SELECT id FROM document_proposals
                 WHERE status = 'accepted' AND landed_revision_id IS NULL
                   AND updated_at < now() - make_interval(mins => $1)
                 ORDER BY updated_at
                 LIMIT $2
                 FOR UPDATE SKIP LOCKED)",
            older_than_minutes,
            limit.max(1),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
