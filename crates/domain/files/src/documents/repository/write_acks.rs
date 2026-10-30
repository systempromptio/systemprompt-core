//! [`DocumentWriteAckRepository`]: the acknowledgement of each write that
//! carried an op id, kept so a retried write is answered, not applied twice.
//!
//! An ack is claimed inside the commit transaction, keyed by document and
//! op id, with the caller and a digest of the request. Two commits racing on
//! one op id serialise on that key: the second waits for the first to
//! commit and then finds its ack. A row without a user predates the digest
//! and is treated as expired.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentOpId, UserId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{AckClaim, StoredWriteAck, WriteAck};

#[derive(Debug, Clone)]
pub struct DocumentWriteAckRepository {
    pools: Pools,
}

struct AckRow {
    user_id: Option<UserId>,
    request_sha256: Option<String>,
    // JSON: the JSONB ack column, decoded into a WriteAck.
    ack: serde_json::Value,
}

fn decode(row: AckRow) -> RepoResult<StoredWriteAck> {
    Ok(StoredWriteAck {
        user_id: row.user_id,
        request_sha256: row.request_sha256,
        ack: serde_json::from_value::<WriteAck>(row.ack)?,
    })
}

impl DocumentWriteAckRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn find(
        &self,
        document: &DocumentId,
        op_id: &DocumentOpId,
    ) -> RepoResult<Option<StoredWriteAck>> {
        sqlx::query_as!(
            AckRow,
            r#"SELECT user_id AS "user_id: UserId", request_sha256, ack
               FROM document_write_acks
               WHERE document_id = $1::text::uuid AND op_id = $2 AND user_id IS NOT NULL"#,
            document.as_str(),
            op_id.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(decode)
        .transpose()
    }

    pub async fn claim(
        conn: &mut PgConnection,
        claim: AckClaim<'_>,
    ) -> RepoResult<Option<StoredWriteAck>> {
        sqlx::query!(
            "DELETE FROM document_write_acks
             WHERE document_id = $1::text::uuid AND op_id = $2 AND user_id IS NULL",
            claim.document.as_str(),
            claim.op_id.as_str(),
        )
        .execute(&mut *conn)
        .await?;
        let ack = serde_json::to_value(claim.ack)?;
        let claimed = sqlx::query_scalar!(
            r#"INSERT INTO document_write_acks
                   (document_id, op_id, revision_id, ack, user_id, request_sha256)
               VALUES ($1::text::uuid, $2, $3::text::uuid, $4, $5, $6)
               ON CONFLICT (document_id, op_id) DO NOTHING
               RETURNING 1 AS "claimed!""#,
            claim.document.as_str(),
            claim.op_id.as_str(),
            claim.ack.revision_id.as_str(),
            ack,
            claim.user.as_str(),
            claim.request_sha256,
        )
        .fetch_optional(&mut *conn)
        .await?;
        if claimed.is_some() {
            return Ok(None);
        }
        sqlx::query_as!(
            AckRow,
            r#"SELECT user_id AS "user_id: UserId", request_sha256, ack
               FROM document_write_acks
               WHERE document_id = $1::text::uuid AND op_id = $2"#,
            claim.document.as_str(),
            claim.op_id.as_str(),
        )
        .fetch_one(&mut *conn)
        .await
        .map(decode)?
        .map(Some)
    }

    pub async fn release(
        &self,
        document: &DocumentId,
        op_id: &DocumentOpId,
        user: &UserId,
    ) -> RepoResult<()> {
        sqlx::query!(
            "DELETE FROM document_write_acks
             WHERE document_id = $1::text::uuid AND op_id = $2 AND user_id = $3",
            document.as_str(),
            op_id.as_str(),
            user.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn sweep_older_than(&self, days: i32, limit: i64) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM document_write_acks
             WHERE (document_id, op_id) IN (
                 SELECT document_id, op_id FROM document_write_acks
                 WHERE created_at < now() - make_interval(days => $1)
                 LIMIT $2)",
            days,
            limit.max(1),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
