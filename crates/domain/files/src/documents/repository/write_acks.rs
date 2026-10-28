//! [`DocumentWriteAckRepository`]: the acknowledgement of each write that
//! carried an op id, kept so a retried write is answered, not applied twice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentOpId, DocumentRevisionId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::StoredWriteAck;

#[derive(Debug, Clone)]
pub struct DocumentWriteAckRepository {
    pools: Pools,
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
        Ok(sqlx::query_as!(
            StoredWriteAck,
            r#"SELECT revision_id::text AS "revision_id!: DocumentRevisionId", ack
               FROM document_write_acks
               WHERE document_id = $1::text::uuid AND op_id = $2"#,
            document.as_str(),
            op_id.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?)
    }

    pub async fn insert(
        &self,
        document: &DocumentId,
        op_id: &DocumentOpId,
        ack: &StoredWriteAck,
    ) -> RepoResult<()> {
        sqlx::query!(
            r#"INSERT INTO document_write_acks (document_id, op_id, revision_id, ack)
               VALUES ($1::text::uuid, $2, $3::text::uuid, $4)
               ON CONFLICT (document_id, op_id) DO NOTHING"#,
            document.as_str(),
            op_id.as_str(),
            ack.revision_id.as_str(),
            ack.ack,
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
