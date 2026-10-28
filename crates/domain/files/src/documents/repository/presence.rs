//! [`DocumentPresenceRepository`]: who is viewing or editing a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::rows::{
    RepoResult, parse_presence_source, parse_presence_state, presence_source_str,
    presence_state_str,
};
use crate::documents::models::{DocumentPresenceRow, PresenceUpsert};

#[derive(Debug, Clone)]
pub struct DocumentPresenceRepository {
    pools: Pools,
}

struct PresenceRow {
    document_id: DocumentId,
    user_id: UserId,
    source: String,
    state: String,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: Option<serde_json::Value>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl TryFrom<PresenceRow> for DocumentPresenceRow {
    type Error = RepositoryError;

    fn try_from(row: PresenceRow) -> RepoResult<Self> {
        Ok(Self {
            source: parse_presence_source(&row.source)?,
            state: parse_presence_state(&row.state)?,
            document_id: row.document_id,
            user_id: row.user_id,
            cursor: row.cursor,
            updated_at: row.updated_at,
            expires_at: row.expires_at,
        })
    }
}

impl DocumentPresenceRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn upsert(&self, presence: &PresenceUpsert, ttl: Duration) -> RepoResult<()> {
        sqlx::query!(
            "INSERT INTO document_presence (document_id, user_id, source, state, cursor, expires_at)
             VALUES ($1::text::uuid, $2, $3, $4, $5, now() + $6::float8 * interval '1 second')
             ON CONFLICT (document_id, user_id, source) DO UPDATE
                 SET state = EXCLUDED.state,
                     cursor = EXCLUDED.cursor,
                     updated_at = now(),
                     expires_at = EXCLUDED.expires_at",
            presence.document_id.as_str(),
            presence.user_id.as_str(),
            presence_source_str(presence.source),
            presence_state_str(presence.state),
            presence.cursor,
            ttl.as_secs_f64(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn list_active(
        &self,
        document_id: &DocumentId,
    ) -> RepoResult<Vec<DocumentPresenceRow>> {
        sqlx::query_as!(
            PresenceRow,
            r#"SELECT document_id::text AS "document_id!: DocumentId",
                      user_id AS "user_id: UserId",
                      source, state, cursor, updated_at, expires_at
               FROM document_presence
               WHERE document_id = $1::text::uuid AND expires_at > now()
               ORDER BY updated_at DESC, user_id, source"#,
            document_id.as_str(),
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentPresenceRow::try_from)
        .collect()
    }

    pub async fn users_present(&self, document_id: &DocumentId) -> RepoResult<Vec<UserId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT DISTINCT user_id AS "user_id!: UserId"
               FROM document_presence
               WHERE document_id = $1::text::uuid AND expires_at > now()
               ORDER BY 1"#,
            document_id.as_str(),
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }

    pub async fn sweep_expired(&self, limit: i64) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM document_presence
             WHERE (document_id, user_id, source) IN (
                 SELECT document_id, user_id, source FROM document_presence
                 WHERE expires_at <= now()
                 LIMIT $1)",
            limit.max(1),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
