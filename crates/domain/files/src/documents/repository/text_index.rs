//! [`DocumentTextIndexRepository`]: the per-head full-text index and a
//! visibility-filtered search over it.
//!
//! The index is written after a revision lands, so two heads in quick
//! succession can finish extracting out of order. A write is applied only
//! while its revision is still the document's head, so the slower, older
//! write never replaces the newer text.
//!
//! A search page applies trash, folder deletion and grant visibility in the
//! statement, before `LIMIT`, and returns each hit with its document row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};

use super::Pools;
use super::documents::DocumentRow;
use super::rows::RepoResult;
use crate::documents::models::{Document, DocumentSearchHit, SubjectPairs};

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
             SELECT d.id, d.head_revision_id, $3, to_tsvector('simple', $3)
             FROM documents d
             WHERE d.id = $1::text::uuid AND d.head_revision_id = $2::text::uuid
             ON CONFLICT (document_id) DO UPDATE
                 SET revision_id = EXCLUDED.revision_id,
                     plain_text = EXCLUDED.plain_text,
                     body = EXCLUDED.body,
                     updated_at = now()
                 WHERE EXISTS (
                     SELECT 1 FROM documents d
                     WHERE d.id = EXCLUDED.document_id
                       AND d.head_revision_id = EXCLUDED.revision_id)",
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
        page: SearchPage,
    ) -> RepoResult<Vec<(Document, DocumentSearchHit)>> {
        let rows = sqlx::query_as!(
            SearchRow,
            r#"SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId",
                      t.revision_id::text AS "revision_id!: DocumentRevisionId",
                      ts_headline('simple', t.plain_text, q) AS "snippet!"
               FROM document_text_index t
               CROSS JOIN plainto_tsquery('simple', $1) AS q
               JOIN documents d ON d.id = t.document_id AND d.deleted_at IS NULL
               LEFT JOIN document_folders f ON f.id = d.folder_id
               WHERE t.body @@ q
                 AND (d.folder_id IS NULL OR f.deleted_at IS NULL)
                 AND (d.owner_user_id = $2 OR EXISTS (
                     SELECT 1 FROM document_grants g
                     WHERE (g.document_id = d.id OR g.folder_id = ANY(f.ancestor_ids || f.id))
                       AND (g.subject_kind, g.subject_value) IN (
                           SELECT * FROM UNNEST($3::text[], $4::text[]))
                       AND (g.valid_until IS NULL OR g.valid_until > now())))
               ORDER BY ts_rank(t.body, q) DESC, t.document_id
               LIMIT $5 OFFSET $6"#,
            query,
            owner.as_str(),
            &subjects.kinds,
            &subjects.values,
            page.limit.max(1),
            page.offset.max(0),
        )
        .fetch_all(self.pools.authz())
        .await?;
        rows.into_iter().map(SearchRow::split).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchPage {
    pub limit: i64,
    pub offset: i64,
}

struct SearchRow {
    id: DocumentId,
    folder_id: Option<DocumentFolderId>,
    name: String,
    format: String,
    mime_type: String,
    head_revision_id: Option<DocumentRevisionId>,
    template_id: Option<DocumentTemplateId>,
    owner_user_id: UserId,
    created_by: UserId,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    trashed_at: Option<DateTime<Utc>>,
    trashed_by: Option<UserId>,
    revision_id: DocumentRevisionId,
    snippet: String,
}

impl SearchRow {
    fn split(self) -> RepoResult<(Document, DocumentSearchHit)> {
        let hit = DocumentSearchHit {
            document_id: self.id.clone(),
            revision_id: self.revision_id,
            snippet: self.snippet,
        };
        let document = Document::try_from(DocumentRow {
            id: self.id,
            folder_id: self.folder_id,
            name: self.name,
            format: self.format,
            mime_type: self.mime_type,
            head_revision_id: self.head_revision_id,
            template_id: self.template_id,
            owner_user_id: self.owner_user_id,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            trashed_at: self.trashed_at,
            trashed_by: self.trashed_by,
        })?;
        Ok((document, hit))
    }
}
