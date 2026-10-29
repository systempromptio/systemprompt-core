//! The web presence half of [`DocumentCollabRepository`]: who is editing a
//! document, their cursors, and the sweep of closed sessions' logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, UserId};

use super::collab_updates::DocumentCollabRepository;
use super::rows::RepoResult;

impl DocumentCollabRepository {
    pub async fn count_editors(&self, document_id: &DocumentId) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM document_presence
               WHERE document_id = $1::text::uuid AND source = 'web'
                 AND state = 'editing' AND expires_at > now()"#,
            document_id.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn is_editing(&self, document_id: &DocumentId, user: &UserId) -> RepoResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM document_presence
                   WHERE document_id = $1::text::uuid AND user_id = $2 AND source = 'web'
                     AND state = 'editing' AND expires_at > now()) AS "editing!""#,
            document_id.as_str(),
            user.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn remove_web_presence(
        &self,
        document_id: &DocumentId,
        user: &UserId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM document_presence
             WHERE document_id = $1::text::uuid AND user_id = $2 AND source = 'web'",
            document_id.as_str(),
            user.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_cursor(
        &self,
        document_id: &DocumentId,
        user: &UserId,
        // JSON: editor cursor position; the shape belongs to the client's editor.
        cursor: &serde_json::Value,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_presence SET cursor = $3, updated_at = now()
             WHERE document_id = $1::text::uuid AND user_id = $2 AND source = 'web'
               AND expires_at > now()",
            document_id.as_str(),
            user.as_str(),
            cursor,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn sweep_closed(&self, limit: i64) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM document_collab_updates
             WHERE (session_id, seq) IN (
                 SELECT u.session_id, u.seq FROM document_collab_updates u
                 JOIN document_edit_sessions s ON s.id = u.session_id
                 WHERE s.closed_at IS NOT NULL
                 LIMIT $1)",
            limit.max(1),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
