//! The image admissions owed to documents whose revisions predate
//! admission records: the `document_media_backfill` queue, the revisions a
//! queued document holds, and the transaction that records its admissions
//! and dequeues it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Acquire;
use systemprompt_identifiers::{ContentHash, DocumentId, UserId};

use super::DocumentMediaRepository;
use super::rows::RepoResult;

impl DocumentMediaRepository {
    pub async fn backfill_queued(&self, document: &DocumentId) -> RepoResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM document_media_backfill
                              WHERE document_id = $1::text::uuid) AS "queued!""#,
            document.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn backfill_batch(&self, limit: i64) -> RepoResult<Vec<DocumentId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT document_id::text AS "document_id!: DocumentId"
               FROM document_media_backfill ORDER BY queued_at, document_id LIMIT $1"#,
            limit,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn revision_sources(
        &self,
        document: &DocumentId,
    ) -> RepoResult<Vec<(ContentHash, UserId)>> {
        let rows = sqlx::query!(
            r#"SELECT content_sha256 AS "content_sha256: ContentHash",
                      (array_agg(author_user_id ORDER BY created_at, id))[1] AS "author!"
               FROM document_revisions WHERE document_id = $1::text::uuid
               GROUP BY content_sha256 ORDER BY MIN(created_at), content_sha256"#,
            document.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.content_sha256, UserId::new(row.author)))
            .collect())
    }

    pub async fn complete_backfill(
        &self,
        document: &DocumentId,
        hashes: &[String],
        admitted_by: &[String],
    ) -> RepoResult<()> {
        let mut conn = self.pools.write.acquire().await?;
        let mut tx = conn.begin().await?;
        sqlx::query!(
            "INSERT INTO document_media (document_id, content_sha256, admitted_by)
             SELECT $1::text::uuid, a.hash, a.admitted_by
               FROM unnest($2::text[], $3::text[]) AS a(hash, admitted_by)
               JOIN document_blobs b ON b.content_sha256 = a.hash
             ON CONFLICT (document_id, content_sha256) DO NOTHING",
            document.as_str(),
            hashes,
            admitted_by,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM document_media_backfill WHERE document_id = $1::text::uuid",
            document.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
