//! [`DocumentLockRepository`]: one expiring lease per document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentLock, LockOutcome, LockRequest};

#[derive(Debug, Clone)]
pub struct DocumentLockRepository {
    pools: Pools,
}

struct LockRow {
    document_id: DocumentId,
    holder_user_id: UserId,
    holder_kind: String,
    holder_id: String,
    acquired_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl TryFrom<LockRow> for DocumentLock {
    type Error = RepositoryError;

    fn try_from(row: LockRow) -> RepoResult<Self> {
        Ok(Self {
            holder_kind: parse("document_locks.holder_kind", &row.holder_kind)?,
            document_id: row.document_id,
            holder_user_id: row.holder_user_id,
            holder_id: row.holder_id,
            acquired_at: row.acquired_at,
            expires_at: row.expires_at,
        })
    }
}

impl DocumentLockRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn acquire(&self, request: &LockRequest) -> RepoResult<LockOutcome> {
        for _ in 0..2 {
            if let Some(lock) = self.try_acquire(request).await? {
                return Ok(LockOutcome::Acquired(lock));
            }
            if let Some(current) = self.current(&request.document_id).await? {
                return Ok(LockOutcome::Held { current });
            }
        }
        Err(RepositoryError::conflict(
            "document lock",
            &request.document_id,
            "lease changed hands while acquiring",
        ))
    }

    async fn try_acquire(&self, request: &LockRequest) -> RepoResult<Option<DocumentLock>> {
        sqlx::query_as!(
            LockRow,
            r#"INSERT INTO document_locks (document_id, holder_user_id, holder_kind, holder_id, expires_at)
               VALUES ($1::text::uuid, $2, $3, $4, now() + $5::float8 * interval '1 second')
               ON CONFLICT (document_id) DO UPDATE
                   SET holder_user_id = EXCLUDED.holder_user_id,
                       holder_kind = EXCLUDED.holder_kind,
                       holder_id = EXCLUDED.holder_id,
                       acquired_at = CASE WHEN document_locks.holder_id = EXCLUDED.holder_id
                                          AND document_locks.expires_at > now()
                                          THEN document_locks.acquired_at ELSE now() END,
                       expires_at = EXCLUDED.expires_at
                   WHERE document_locks.holder_id = EXCLUDED.holder_id
                      OR document_locks.expires_at <= now()
               RETURNING document_id::text AS "document_id!: DocumentId",
                         holder_user_id AS "holder_user_id: UserId",
                         holder_kind, holder_id, acquired_at, expires_at"#,
            request.document_id.as_str(),
            request.holder_user_id.as_str(),
            request.holder_kind.as_str(),
            request.holder_id,
            request.ttl.as_secs_f64(),
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentLock::try_from)
        .transpose()
    }

    pub async fn release(&self, document_id: &DocumentId, holder_id: &str) -> RepoResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM document_locks WHERE document_id = $1::text::uuid AND holder_id = $2",
            document_id.as_str(),
            holder_id,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn release_all(&self, document_id: &DocumentId) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM document_locks WHERE document_id = $1::text::uuid",
            document_id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn current(&self, document_id: &DocumentId) -> RepoResult<Option<DocumentLock>> {
        sqlx::query_as!(
            LockRow,
            r#"SELECT document_id::text AS "document_id!: DocumentId",
                      holder_user_id AS "holder_user_id: UserId",
                      holder_kind, holder_id, acquired_at, expires_at
               FROM document_locks
               WHERE document_id = $1::text::uuid AND expires_at > now()"#,
            document_id.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentLock::try_from)
        .transpose()
    }
}
