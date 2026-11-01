//! [`DocumentBlobRepository`]: content-addressed blob rows and their
//! garbage-collection candidates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContentHash;
use systemprompt_traits::StoredFileId;

use super::Pools;
use super::blob_holders::held_by_holders;
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
        storage_id: &StoredFileId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "INSERT INTO document_blobs (content_sha256, size_bytes, storage_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (content_sha256) DO NOTHING",
            hash.as_str(),
            size_bytes,
            storage_id.as_str(),
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
                      size_bytes, storage_id AS "storage_id: StoredFileId", created_at
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
        let wanted = usize::try_from(limit.max(1)).unwrap_or(usize::MAX);
        let hours = i32::try_from(interval_hours).unwrap_or(i32::MAX);
        let mut conn = self.pools.read.acquire().await?;
        let mut collected: Vec<DocumentBlob> = Vec::new();
        let mut cursor: Option<(DateTime<Utc>, String)> = None;
        loop {
            let (after_at, after_hash) = match &cursor {
                Some((at, hash)) => (Some(*at), Some(hash.as_str())),
                None => (None, None),
            };
            let page = sqlx::query_as!(
                DocumentBlob,
                r#"SELECT b.content_sha256 AS "content_sha256: ContentHash",
                          b.size_bytes, b.storage_id AS "storage_id: StoredFileId", b.created_at
                   FROM document_blobs b
                   WHERE b.created_at < now() - make_interval(hours => $1::int)
                     AND ($3::timestamptz IS NULL
                          OR (b.created_at, b.content_sha256) > ($3, $4::text))
                     AND NOT EXISTS (SELECT 1 FROM document_revisions r
                                     WHERE r.content_sha256 = b.content_sha256)
                     AND NOT EXISTS (SELECT 1 FROM document_edit_sessions s
                                     WHERE s.snapshot_sha256 = b.content_sha256)
                   ORDER BY b.created_at, b.content_sha256
                   LIMIT $2"#,
                hours,
                i64::try_from(wanted).unwrap_or(i64::MAX),
                after_at,
                after_hash,
            )
            .fetch_all(&mut *conn)
            .await?;
            let full_page = page.len() >= wanted;
            let last = page
                .last()
                .map(|blob| (blob.created_at, blob.content_sha256.as_str().to_owned()));
            let hashes: Vec<String> = page
                .iter()
                .map(|blob| blob.content_sha256.as_str().to_owned())
                .collect();
            let held = held_by_holders(&mut conn, &hashes).await?;
            collected.extend(
                page.into_iter()
                    .filter(|blob| !held.contains(blob.content_sha256.as_str())),
            );
            if collected.len() >= wanted || !full_page {
                collected.truncate(wanted);
                return Ok(collected);
            }
            cursor = last;
        }
    }

    pub async fn delete(&self, hash: &ContentHash) -> RepoResult<bool> {
        let mut conn = self.pools.write.acquire().await?;
        let held = held_by_holders(&mut conn, &[hash.as_str().to_owned()]).await?;
        if !held.is_empty() {
            return Ok(false);
        }
        let result = sqlx::query!(
            "DELETE FROM document_blobs b WHERE b.content_sha256 = $1
               AND NOT EXISTS (SELECT 1 FROM document_revisions r
                               WHERE r.content_sha256 = b.content_sha256)
               AND NOT EXISTS (SELECT 1 FROM document_edit_sessions s
                               WHERE s.snapshot_sha256 = b.content_sha256)",
            hash.as_str(),
        )
        .execute(&mut *conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
