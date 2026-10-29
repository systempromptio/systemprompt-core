//! [`DocumentRepository`]: document rows and the compare-and-swap head
//! revision. A document created in a folder takes that folder's tree lock and
//! requires the folder to be live.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::folder_locks::begin_tree_locked;
use super::rows::{RepoResult, parse};
use crate::documents::models::{Document, NewDocument};

#[derive(Debug, Clone)]
pub struct DocumentRepository {
    pub(super) pools: Pools,
}

pub(super) struct DocumentRow {
    pub(super) id: DocumentId,
    pub(super) folder_id: Option<DocumentFolderId>,
    pub(super) name: String,
    pub(super) format: String,
    pub(super) mime_type: String,
    pub(super) head_revision_id: Option<DocumentRevisionId>,
    pub(super) template_id: Option<DocumentTemplateId>,
    pub(super) owner_user_id: UserId,
    pub(super) created_by: UserId,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) trashed_at: Option<DateTime<Utc>>,
    pub(super) trashed_by: Option<UserId>,
}

impl TryFrom<DocumentRow> for Document {
    type Error = RepositoryError;

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
            trashed_at: row.trashed_at,
            trashed_by: row.trashed_by,
        })
    }
}

impl DocumentRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewDocument) -> RepoResult<Document> {
        let folder: Vec<&str> = request
            .folder_id
            .iter()
            .map(DocumentFolderId::as_str)
            .collect();
        let mut tx = begin_tree_locked(&self.pools.write, &folder).await?;
        let row = sqlx::query_as!(
            DocumentRow,
            r#"INSERT INTO documents (folder_id, name, format, mime_type, template_id, owner_user_id, created_by)
               SELECT $1::text::uuid, $2, $3, $4, $5, $6, $7
               WHERE $1::text IS NULL OR EXISTS (
                   SELECT 1 FROM document_folders f
                   WHERE f.id = $1::text::uuid AND f.deleted_at IS NULL)
               RETURNING id::text AS "id!: DocumentId",
                         folder_id::text AS "folder_id: DocumentFolderId",
                         name, format, mime_type,
                         head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                         template_id AS "template_id: DocumentTemplateId",
                         owner_user_id AS "owner_user_id: UserId",
                         created_by AS "created_by: UserId",
                         created_at, updated_at, deleted_at AS trashed_at,
                         trashed_by AS "trashed_by: UserId""#,
            request.folder_id.as_ref().map(DocumentFolderId::as_str),
            request.name,
            request.format.as_str(),
            request.mime_type,
            request.template_id.as_ref().map(DocumentTemplateId::as_str),
            request.owner_user_id.as_str(),
            request.created_by.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            RepositoryError::not_found(
                "document folder",
                request
                    .folder_id
                    .as_ref()
                    .map_or("<root>", DocumentFolderId::as_str),
            )
        })?;
        tx.commit().await?;
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
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId"
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
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId"
               FROM documents d WHERE d.folder_id IS NOT DISTINCT FROM $1::text::uuid AND d.name = $2 AND d.deleted_at IS NULL"#,
            folder.map(DocumentFolderId::as_str),
            name
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .map(Document::try_from)
        .transpose()
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
