//! [`DocumentFolderRepository`]: the folder tree with materialised ancestors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_models::documents::AgentEditPolicy;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::DocumentFolder;

pub(super) struct FolderRow {
    pub(super) id: DocumentFolderId,
    pub(super) parent_id: Option<DocumentFolderId>,
    pub(super) name: String,
    pub(super) ancestor_ids: Vec<DocumentFolderId>,
    pub(super) owner_user_id: UserId,
    pub(super) created_by: UserId,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) deleted_at: Option<DateTime<Utc>>,
    pub(super) agent_edit_policy: String,
}

impl TryFrom<FolderRow> for DocumentFolder {
    type Error = systemprompt_traits::RepositoryError;

    fn try_from(row: FolderRow) -> RepoResult<Self> {
        Ok(Self {
            agent_edit_policy: parse("document_folders.agent_edit_policy", &row.agent_edit_policy)?,
            id: row.id,
            parent_id: row.parent_id,
            name: row.name,
            ancestor_ids: row.ancestor_ids,
            owner_user_id: row.owner_user_id,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            trashed_at: row.deleted_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DocumentFolderRepository {
    pub(super) pools: Pools,
}

impl DocumentFolderRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn find(&self, id: &DocumentFolderId) -> RepoResult<Option<DocumentFolder>> {
        sqlx::query_as!(
            FolderRow,
            r#"SELECT id::text AS "id!: DocumentFolderId",
                      parent_id::text AS "parent_id: DocumentFolderId",
                      name,
                      ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      owner_user_id AS "owner_user_id: UserId",
                      created_by AS "created_by: UserId",
                      created_at, updated_at, deleted_at, agent_edit_policy
               FROM document_folders WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
            id.as_str()
        )
        .fetch_optional(self.pools.authz())
        .await?
        .map(DocumentFolder::try_from)
        .transpose()
    }

    pub async fn list_children(
        &self,
        parent: Option<&DocumentFolderId>,
    ) -> RepoResult<Vec<DocumentFolder>> {
        sqlx::query_as!(
            FolderRow,
r#"SELECT id::text AS "id!: DocumentFolderId",
                      parent_id::text AS "parent_id: DocumentFolderId",
                      name,
                      ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      owner_user_id AS "owner_user_id: UserId",
                      created_by AS "created_by: UserId",
                      created_at, updated_at, deleted_at, agent_edit_policy
               FROM document_folders WHERE parent_id IS NOT DISTINCT FROM $1::text::uuid AND deleted_at IS NULL ORDER BY name, id"#,
            parent.map(DocumentFolderId::as_str)
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(DocumentFolder::try_from)
        .collect()
    }

    pub async fn set_agent_edit_policy(
        &self,
        id: &DocumentFolderId,
        policy: AgentEditPolicy,
    ) -> RepoResult<bool> {
        set_agent_edit_policy_in(&*self.pools.write, id, policy).await
    }

    pub async fn effective_agent_edit_policy(
        &self,
        id: &DocumentFolderId,
    ) -> RepoResult<AgentEditPolicy> {
        let proposes = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM document_folders f
                   JOIN document_folders a
                     ON a.id = f.id OR a.id = ANY(f.ancestor_ids)
                   WHERE f.id = $1::text::uuid
                     AND a.deleted_at IS NULL
                     AND a.agent_edit_policy = 'propose'
               ) AS "proposes!""#,
            id.as_str(),
        )
        .fetch_one(self.pools.authz())
        .await?;
        Ok(if proposes {
            AgentEditPolicy::Propose
        } else {
            AgentEditPolicy::Direct
        })
    }

    pub async fn soft_delete(&self, id: &DocumentFolderId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_folders SET deleted_at = now(), updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
            id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn ancestors(&self, id: &DocumentFolderId) -> RepoResult<Vec<DocumentFolderId>> {
        let row = sqlx::query_scalar!(
            r#"SELECT ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>"
               FROM document_folders WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(self.pools.authz())
        .await?;
        row.ok_or_else(|| systemprompt_traits::RepositoryError::not_found("document folder", id))
    }

    pub async fn path(&self, id: &DocumentFolderId) -> RepoResult<String> {
        let path = sqlx::query_scalar!(
            r#"SELECT string_agg(a.name, '/' ORDER BY u.ord) AS "path"
               FROM document_folders f
               CROSS JOIN LATERAL unnest(f.ancestor_ids || f.id) WITH ORDINALITY AS u(folder_id, ord)
               JOIN document_folders a ON a.id = u.folder_id
               WHERE f.id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        path.ok_or_else(|| systemprompt_traits::RepositoryError::not_found("document folder", id))
    }
}

pub(super) async fn set_agent_edit_policy_in<'e, E>(
    executor: E,
    id: &DocumentFolderId,
    policy: AgentEditPolicy,
) -> RepoResult<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let result = sqlx::query!(
        "UPDATE document_folders SET agent_edit_policy = $2, updated_at = now()
             WHERE id = $1::text::uuid AND deleted_at IS NULL",
        id.as_str(),
        policy.as_str(),
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}
