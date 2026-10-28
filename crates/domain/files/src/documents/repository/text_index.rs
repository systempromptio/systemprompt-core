//! [`DocumentTextIndexRepository`]: the per-head full-text index and a
//! visibility-filtered search over it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, UserId};

use super::Pools;
use super::rows::RepoResult;
use crate::documents::models::{DocumentSearchHit, SubjectPairs};

#[derive(Debug, Clone)]
pub struct DocumentTextIndexRepository {
    pools: Pools,
}

impl DocumentTextIndexRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn upsert(
        &self,
        document_id: &DocumentId,
        revision_id: &DocumentRevisionId,
        text: &str,
    ) -> RepoResult<()> {
        sqlx::query!(
            "INSERT INTO document_text_index (document_id, revision_id, plain_text, body)
             VALUES ($1::text::uuid, $2::text::uuid, $3, to_tsvector('simple', $3))
             ON CONFLICT (document_id) DO UPDATE
                 SET revision_id = EXCLUDED.revision_id,
                     plain_text = EXCLUDED.plain_text,
                     body = EXCLUDED.body,
                     updated_at = now()",
            document_id.as_str(),
            revision_id.as_str(),
            text,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }

    pub async fn search(
        &self,
        query: &str,
        subjects: &SubjectPairs,
        owner: &UserId,
        limit: i64,
    ) -> RepoResult<Vec<DocumentSearchHit>> {
        Ok(sqlx::query_as!(
            DocumentSearchHit,
            r#"SELECT t.document_id::text AS "document_id!: DocumentId",
                      t.revision_id::text AS "revision_id!: DocumentRevisionId",
                      ts_headline('simple', t.plain_text, q) AS "snippet!"
               FROM document_text_index t
               CROSS JOIN plainto_tsquery('simple', $1) AS q
               JOIN documents d ON d.id = t.document_id AND d.deleted_at IS NULL
               LEFT JOIN document_folders f ON f.id = d.folder_id
               WHERE t.body @@ q
                 AND (d.owner_user_id = $2 OR EXISTS (
                     SELECT 1 FROM document_grants g
                     WHERE (g.document_id = d.id OR g.folder_id = ANY(f.ancestor_ids || f.id))
                       AND (g.subject_kind, g.subject_value) IN (
                           SELECT * FROM UNNEST($3::text[], $4::text[]))
                       AND (g.valid_until IS NULL OR g.valid_until > now())))
               ORDER BY ts_rank(t.body, q) DESC, t.document_id
               LIMIT $5"#,
            query,
            owner.as_str(),
            &subjects.kinds,
            &subjects.values,
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }
}
