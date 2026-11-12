//! [`DocumentMediaRepository`]: the uploaded images admitted to each
//! document. Serving an image requires its row; admitting one happens inside
//! the landing transaction of the revision that places it, which refuses
//! the landing when an admitted image's blob has been collected meanwhile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentHash, DocumentId, UserId};

use super::Pools;
use super::rows::RepoResult;

#[derive(Debug, Clone)]
pub struct DocumentMediaRepository {
    pub(super) pools: Pools,
}

impl DocumentMediaRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn is_admitted(&self, document: &DocumentId, hash: &ContentHash) -> RepoResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM document_media
                              WHERE document_id = $1::text::uuid AND content_sha256 = $2)
                   AS "admitted!""#,
            document.as_str(),
            hash.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?)
    }

    pub async fn admitted_among(
        &self,
        document: &DocumentId,
        hashes: &[String],
    ) -> RepoResult<Vec<String>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }
        Ok(sqlx::query_scalar!(
            "SELECT content_sha256 FROM document_media
             WHERE document_id = $1::text::uuid AND content_sha256 = ANY($2)",
            document.as_str(),
            hashes,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn of_document(&self, document: &DocumentId) -> RepoResult<Vec<String>> {
        Ok(sqlx::query_scalar!(
            "SELECT content_sha256 FROM document_media WHERE document_id = $1::text::uuid
             ORDER BY content_sha256",
            document.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn stored_among(&self, hashes: &[String]) -> RepoResult<Vec<String>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }
        Ok(sqlx::query_scalar!(
            "SELECT content_sha256 FROM document_blobs WHERE content_sha256 = ANY($1)",
            hashes,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn admit_in(
        conn: &mut PgConnection,
        document: &DocumentId,
        hashes: &[String],
        admitted_by: &UserId,
    ) -> RepoResult<Vec<String>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }
        let stored = sqlx::query_scalar!(
            "SELECT content_sha256 FROM document_blobs WHERE content_sha256 = ANY($1)
             FOR KEY SHARE",
            hashes,
        )
        .fetch_all(&mut *conn)
        .await?;
        let missing: Vec<String> = hashes
            .iter()
            .filter(|hash| !stored.contains(hash))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Ok(missing);
        }
        sqlx::query!(
            "INSERT INTO document_media (document_id, content_sha256, admitted_by)
             SELECT $1::text::uuid, h, $3 FROM unnest($2::text[]) AS h
             ON CONFLICT (document_id, content_sha256) DO NOTHING",
            document.as_str(),
            hashes,
            admitted_by.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(Vec::new())
    }
}
