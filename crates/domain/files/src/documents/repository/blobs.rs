//! [`DocumentBlobRepository`]: content-addressed blob rows and their
//! garbage-collection candidates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContentHash;

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::DocumentBlob;

#[derive(Debug, Clone)]
pub struct DocumentBlobRepository {
    pools: Pools,
}

impl DocumentBlobRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn upsert(
        &self,
        hash: &ContentHash,
        size_bytes: i64,
        storage_id: &str,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "INSERT INTO document_blobs (content_sha256, size_bytes, storage_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (content_sha256) DO NOTHING",
            hash.as_str(),
            size_bytes,
            storage_id,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn find(&self, hash: &ContentHash) -> RepoResult<Option<DocumentBlob>> {
        Self::find_in(&self.pools.read, hash).await
    }

    pub async fn find_on_primary(&self, hash: &ContentHash) -> RepoResult<Option<DocumentBlob>> {
        Self::find_in(&self.pools.write, hash).await
    }

    async fn find_in(pool: &PgPool, hash: &ContentHash) -> RepoResult<Option<DocumentBlob>> {
        Ok(sqlx::query_as!(
            DocumentBlob,
            r#"SELECT content_sha256 AS "content_sha256: ContentHash",
                      size_bytes, storage_id, created_at
               FROM document_blobs WHERE content_sha256 = $1"#,
            hash.as_str(),
        )
        .fetch_optional(pool)
        .await?)
    }

    pub async fn unreferenced_older_than(
        &self,
        interval_hours: i64,
        limit: i64,
    ) -> RepoResult<Vec<DocumentBlob>> {
        Ok(sqlx::query_as!(
            DocumentBlob,
            r#"SELECT b.content_sha256 AS "content_sha256: ContentHash",
                      b.size_bytes, b.storage_id, b.created_at
               FROM document_blobs b
               WHERE b.created_at < now() - make_interval(hours => $1::int)
                 AND NOT EXISTS (SELECT 1 FROM document_revisions r
                                 WHERE r.content_sha256 = b.content_sha256)
                 AND NOT EXISTS (SELECT 1 FROM document_edit_sessions s
                                 WHERE s.snapshot_sha256 = b.content_sha256)
               ORDER BY b.created_at
               LIMIT $2"#,
            i32::try_from(interval_hours).unwrap_or(i32::MAX),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }

    pub async fn delete(&self, hash: &ContentHash) -> RepoResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM document_blobs b WHERE b.content_sha256 = $1
               AND NOT EXISTS (SELECT 1 FROM document_revisions r
                               WHERE r.content_sha256 = b.content_sha256)
               AND NOT EXISTS (SELECT 1 FROM document_edit_sessions s
                               WHERE s.snapshot_sha256 = b.content_sha256)",
            hash.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
