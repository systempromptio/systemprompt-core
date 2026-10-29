//! [`DocumentCollabRepository`]: the CRDT update log of an edit session, the
//! sequence number its stored snapshot covers, and the web presence rows the
//! co-editing service joins and leaves.
//!
//! An update's sequence number is drawn from its session row under a row
//! lock in the transaction that logs it, so sequence order is commit order
//! and a reader that has seen `seq` has seen every update below it.
//!
//! A snapshot only moves forward: [`DocumentCollabRepository::set_snapshot`]
//! records a snapshot covering `seq`, with the version vector it encodes,
//! only when the stored one covers less, and prunes the updates the new
//! snapshot already contains in the same transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentHash, DocumentEditSessionId, DocumentId, UserId};

use super::Pools;
use super::rows::RepoResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollabUpdateRow {
    pub seq: i64,
    pub update: Vec<u8>,
    pub author_user_id: UserId,
}

#[derive(Debug, Clone, Copy)]
pub struct SnapshotMark<'a> {
    pub hash: &'a ContentHash,
    pub seq: i64,
    pub version: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct DocumentCollabRepository {
    pools: Pools,
}

impl DocumentCollabRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn append(
        &self,
        session: &DocumentEditSessionId,
        update: &[u8],
        author: &UserId,
    ) -> RepoResult<Option<i64>> {
        let mut tx = self.pools.write.begin().await?;
        let Some(seq) = sqlx::query_scalar!(
            "SELECT next_seq FROM document_edit_sessions
             WHERE id = $1::text::uuid AND closed_at IS NULL
             FOR UPDATE",
            session.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        sqlx::query!(
            "UPDATE document_edit_sessions SET next_seq = next_seq + 1
             WHERE id = $1::text::uuid",
            session.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO document_collab_updates (session_id, seq, payload, author_user_id)
             VALUES ($1::text::uuid, $2, $3, $4)",
            session.as_str(),
            seq,
            update,
            author.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(seq))
    }

    pub async fn list_after(
        &self,
        session: &DocumentEditSessionId,
        after_seq: i64,
    ) -> RepoResult<Vec<CollabUpdateRow>> {
        Ok(sqlx::query_as!(
            CollabUpdateRow,
            r#"SELECT seq, payload AS "update", author_user_id AS "author_user_id: UserId"
               FROM document_collab_updates
               WHERE session_id = $1::text::uuid AND seq > $2
               ORDER BY seq"#,
            session.as_str(),
            after_seq,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn snapshot_seq(&self, session: &DocumentEditSessionId) -> RepoResult<Option<i64>> {
        Ok(sqlx::query_scalar!(
            "SELECT snapshot_seq FROM document_edit_sessions WHERE id = $1::text::uuid",
            session.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn set_snapshot(
        &self,
        session: &DocumentEditSessionId,
        snapshot: SnapshotMark<'_>,
    ) -> RepoResult<bool> {
        let SnapshotMark { hash, seq, version } = snapshot;
        let mut tx = self.pools.write.begin().await?;
        let moved = sqlx::query!(
            "UPDATE document_edit_sessions
             SET snapshot_sha256 = $2, snapshot_seq = $3, snapshot_vv = $4, updated_at = now()
             WHERE id = $1::text::uuid AND closed_at IS NULL AND snapshot_seq < $3",
            session.as_str(),
            hash.as_str(),
            seq,
            version,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if moved {
            sqlx::query!(
                "DELETE FROM document_collab_updates
                 WHERE session_id = $1::text::uuid AND seq <= $2",
                session.as_str(),
                seq,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(moved)
    }

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
