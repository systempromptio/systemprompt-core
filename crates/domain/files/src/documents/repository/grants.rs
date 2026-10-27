//! [`DocumentGrantRepository`]: subject-keyed grants on documents and
//! folders, and the effective level a set of subject pairs holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};
use systemprompt_models::documents::AccessLevel;
use systemprompt_traits::RepositoryError;
use uuid::Uuid;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentGrant, GrantTarget, NewGrant, SubjectPairs};

#[derive(Debug, Clone)]
pub struct DocumentGrantRepository {
    pools: Pools,
}

struct GrantRow {
    id: Uuid,
    document_id: Option<DocumentId>,
    folder_id: Option<DocumentFolderId>,
    subject_kind: String,
    subject_value: String,
    access_level: String,
    granted_by: UserId,
    valid_until: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    subject_display: Option<String>,
}

impl TryFrom<GrantRow> for DocumentGrant {
    type Error = RepositoryError;

    fn try_from(row: GrantRow) -> RepoResult<Self> {
        let target = match (row.document_id, row.folder_id) {
            (Some(document), None) => GrantTarget::Document(document),
            (None, Some(folder)) => GrantTarget::Folder(folder),
            _ => {
                return Err(RepositoryError::invalid_data(
                    "document_grants.target",
                    "exactly one of document_id and folder_id must be set",
                ));
            },
        };
        Ok(Self {
            access_level: parse("document_grants.access_level", &row.access_level)?,
            id: row.id,
            target,
            subject_kind: row.subject_kind,
            subject_value: row.subject_value,
            granted_by: row.granted_by,
            valid_until: row.valid_until,
            created_at: row.created_at,
            updated_at: row.updated_at,
            subject_display: row.subject_display,
        })
    }
}

const fn level_from_rank(rank: i32) -> Option<AccessLevel> {
    match rank {
        1 => Some(AccessLevel::View),
        2 => Some(AccessLevel::Comment),
        3 => Some(AccessLevel::Edit),
        4 => Some(AccessLevel::Share),
        5 => Some(AccessLevel::Own),
        _ => None,
    }
}

impl DocumentGrantRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn insert(&self, request: NewGrant) -> RepoResult<DocumentGrant> {
        let (document_id, folder_id) = match &request.target {
            GrantTarget::Document(id) => (Some(id.as_str()), None),
            GrantTarget::Folder(id) => (None, Some(id.as_str())),
        };
        let row = sqlx::query_as!(
            GrantRow,
            r#"WITH g AS (INSERT INTO document_grants
                   (document_id, folder_id, subject_kind, subject_value, subject_display,
                    access_level, granted_by, valid_until)
               VALUES ($1::text::uuid, $2::text::uuid, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (document_id, folder_id, subject_kind, subject_value) DO UPDATE
                   SET access_level = EXCLUDED.access_level,
                       valid_until = EXCLUDED.valid_until,
                       granted_by = EXCLUDED.granted_by,
                       subject_display = COALESCE(EXCLUDED.subject_display,
                                                  document_grants.subject_display),
                       updated_at = now()
               RETURNING *)
               SELECT g.id AS "id!", g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind AS "subject_kind!", g.subject_value AS "subject_value!",
                      g.access_level AS "access_level!",
                      g.granted_by AS "granted_by!: UserId",
                      g.valid_until, g.created_at AS "created_at!", g.updated_at AS "updated_at!",
                      g.subject_display
               FROM g"#,
            document_id,
            folder_id,
            request.subject_kind,
            request.subject_value,
            request.subject_display,
            request.access_level.as_str(),
            request.granted_by.as_str(),
            request.valid_until,
        )
        .fetch_one(&*self.pools.write)
        .await?;
        row.try_into()
    }

    pub async fn delete(&self, id: Uuid) -> RepoResult<bool> {
        let result = sqlx::query!("DELETE FROM document_grants WHERE id = $1", id)
            .execute(&*self.pools.write)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_for_document(&self, id: &DocumentId) -> RepoResult<Vec<DocumentGrant>> {
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.document_id = $1::text::uuid ORDER BY g.created_at, g.id"#,
            id.as_str()
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn list_for_folder(&self, id: &DocumentFolderId) -> RepoResult<Vec<DocumentGrant>> {
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.folder_id = $1::text::uuid ORDER BY g.created_at, g.id"#,
            id.as_str()
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn list_for_folders(
        &self,
        folders: &[DocumentFolderId],
    ) -> RepoResult<Vec<DocumentGrant>> {
        let folders: Vec<String> = folders.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query_as!(
            GrantRow,
            r#"SELECT g.id, g.document_id::text AS "document_id: DocumentId",
                      g.folder_id::text AS "folder_id: DocumentFolderId",
                      g.subject_kind, g.subject_value, g.access_level,
                      g.granted_by AS "granted_by: UserId",
                      g.valid_until, g.created_at, g.updated_at,
                      g.subject_display
               FROM document_grants g
               WHERE g.folder_id = ANY($1::text[]::uuid[]) ORDER BY g.created_at, g.id"#,
            &folders
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentGrant::try_from)
        .collect()
    }

    pub async fn delete_for_document(&self, id: &DocumentId) -> RepoResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM document_grants WHERE document_id = $1::text::uuid",
            id.as_str()
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn effective_level(
        &self,
        document_id: &DocumentId,
        folder_ancestors: &[DocumentFolderId],
        subjects: &SubjectPairs,
    ) -> RepoResult<Option<AccessLevel>> {
        let folders: Vec<String> = folder_ancestors
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let rank = sqlx::query_scalar!(
            r#"SELECT MAX(CASE access_level
                          WHEN 'view' THEN 1 WHEN 'comment' THEN 2 WHEN 'edit' THEN 3
                          WHEN 'share' THEN 4 WHEN 'own' THEN 5 END) AS "rank"
               FROM document_grants g
               WHERE (g.document_id = $1::text::uuid OR g.folder_id = ANY($2::text[]::uuid[]))
                 AND (g.subject_kind, g.subject_value) IN (
                     SELECT * FROM UNNEST($3::text[], $4::text[]))
                 AND (g.valid_until IS NULL OR g.valid_until > now())"#,
            document_id.as_str(),
            &folders,
            &subjects.kinds,
            &subjects.values,
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok(rank.and_then(level_from_rank))
    }
}
