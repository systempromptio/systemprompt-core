//! [`DocumentRepository`]: document rows, visible listings and the
//! compare-and-swap head revision.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use uuid::Uuid;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{Document, ListCursor, NewDocument, SubjectPairs, VisibleListing};

#[derive(Debug, Clone)]
pub struct DocumentRepository {
    pools: Pools,
}

struct DocumentRow {
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
    deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<DocumentRow> for Document {
    type Error = systemprompt_traits::RepositoryError;

    fn try_from(row: DocumentRow) -> RepoResult<Self> {
        Ok(Self {
            format: parse("documents.format", &row.format)?,
            id: row.id,
            folder_id: row.folder_id,
            name: row.name,
            mime_type: row.mime_type,
            head_revision_id: row.head_revision_id,
            template_id: row.template_id,
            owner_user_id: row.owner_user_id,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        })
    }
}

fn convert(rows: Vec<DocumentRow>) -> RepoResult<Vec<Document>> {
    rows.into_iter().map(Document::try_from).collect()
}

impl DocumentRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewDocument) -> RepoResult<Document> {
        let row = sqlx::query_as!(
            DocumentRow,
            r#"INSERT INTO documents (folder_id, name, format, mime_type, template_id, owner_user_id, created_by)
               VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7)
               RETURNING id::text AS "id!: DocumentId",
                         folder_id::text AS "folder_id: DocumentFolderId",
                         name, format, mime_type,
                         head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                         template_id AS "template_id: DocumentTemplateId",
                         owner_user_id AS "owner_user_id: UserId",
                         created_by AS "created_by: UserId",
                         created_at, updated_at, deleted_at"#,
            request.folder_id.as_ref().map(DocumentFolderId::as_str),
            request.name,
            request.format.as_str(),
            request.mime_type,
            request.template_id.as_ref().map(DocumentTemplateId::as_str),
            request.owner_user_id.as_str(),
            request.created_by.as_str(),
        )
        .fetch_one(&*self.pools.write)
        .await?;
        row.try_into()
    }

    pub async fn find(&self, id: &DocumentId) -> RepoResult<Option<Document>> {
        sqlx::query_as!(
            DocumentRow,
            r#"SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at
               FROM documents d WHERE d.id = $1::text::uuid AND d.deleted_at IS NULL"#,
            id.as_str()
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .map(Document::try_from)
        .transpose()
    }

    pub async fn find_in_folder(
        &self,
        folder: Option<&DocumentFolderId>,
        name: &str,
    ) -> RepoResult<Option<Document>> {
        sqlx::query_as!(
DocumentRow,
r#"SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at
               FROM documents d WHERE d.folder_id IS NOT DISTINCT FROM $1::text::uuid AND d.name = $2 AND d.deleted_at IS NULL"#,
            folder.map(DocumentFolderId::as_str),
            name
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .map(Document::try_from)
        .transpose()
    }

    pub async fn list_visible(
        &self,
        listing: VisibleListing,
        subjects: &SubjectPairs,
        owner: &UserId,
    ) -> RepoResult<(Vec<Document>, Option<ListCursor>)> {
        let limit = listing.limit.max(1);
        let pattern = listing.query.as_deref().map(|q| {
            format!(
                "%{}%",
                q.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        });
        let rows = sqlx::query_as!(
            DocumentRow,
            r#"SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at
               FROM documents d LEFT JOIN document_folders f ON f.id = d.folder_id
               WHERE d.deleted_at IS NULL
                 AND ($1::text IS NULL OR d.folder_id = $1::text::uuid)
                 AND ($2::text IS NULL OR d.name ILIKE $2)
                 AND ($3::timestamptz IS NULL OR (d.updated_at, d.id) < ($3, $4::uuid))
                 AND (d.owner_user_id = $5 OR EXISTS (
                     SELECT 1 FROM document_grants g
                     WHERE (g.document_id = d.id OR g.folder_id = ANY(f.ancestor_ids || f.id))
                       AND (g.subject_kind, g.subject_value) IN (
                           SELECT * FROM UNNEST($6::text[], $7::text[]))
                       AND (g.valid_until IS NULL OR g.valid_until > now())))
               ORDER BY d.updated_at DESC, d.id DESC
               LIMIT $8"#,
            listing.folder_id.as_ref().map(DocumentFolderId::as_str),
            pattern,
            listing.cursor.map(|c| c.updated_at),
            listing.cursor.map(|c| c.id),
            owner.as_str(),
            &subjects.kinds,
            &subjects.values,
            limit + 1,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        let mut documents = convert(rows)?;
        let next = if documents.len() as i64 > limit {
            documents.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
            documents.last().map(cursor_of).transpose()?
        } else {
            None
        };
        Ok((documents, next))
    }

    pub async fn rename(&self, id: &DocumentId, name: &str) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET name = $2, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
            name,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn move_to(
        &self,
        id: &DocumentId,
        folder: Option<&DocumentFolderId>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET folder_id = $2::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
            folder.map(DocumentFolderId::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn soft_delete(&self, id: &DocumentId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET deleted_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_head_if(
        &self,
        id: &DocumentId,
        expected: Option<&DocumentRevisionId>,
        new: &DocumentRevisionId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET head_revision_id = $3::text::uuid, updated_at = now()
             WHERE id = $1::text::uuid
               AND head_revision_id IS NOT DISTINCT FROM $2::text::uuid
               AND deleted_at IS NULL
               AND EXISTS (SELECT 1 FROM document_revisions r
                           WHERE r.id = $3::text::uuid AND r.document_id = $1::text::uuid)",
            id.as_str(),
            expected.map(DocumentRevisionId::as_str),
            new.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_template(
        &self,
        id: &DocumentId,
        template_id: Option<&DocumentTemplateId>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE documents SET template_id = $2, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
            template_id.map(DocumentTemplateId::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

fn cursor_of(document: &Document) -> RepoResult<ListCursor> {
    let id = Uuid::parse_str(document.id.as_str())
        .map_err(|source| systemprompt_traits::RepositoryError::decode("documents.id", source))?;
    Ok(ListCursor::new(document.updated_at, id))
}
