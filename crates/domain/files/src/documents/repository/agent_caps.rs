//! Agent cap overrides on documents and folders, and the audit of agent
//! reads made under a cap below `edit`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId, UserId};

use super::DocumentRepository;
use super::rows::{RepoResult, parse};
use crate::documents::agents::AgentCap;

/// One override set on a target or a folder above it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapOverride {
    Document(DocumentId, AgentCap),
    Folder(DocumentFolderId, AgentCap),
}

impl CapOverride {
    #[must_use]
    pub const fn cap(&self) -> AgentCap {
        match self {
            Self::Document(_, cap) | Self::Folder(_, cap) => *cap,
        }
    }
}

/// One agent read to audit.
#[derive(Debug, Clone, Copy)]
pub struct AgentReadRecord<'a> {
    pub document_id: &'a DocumentId,
    pub user_id: &'a UserId,
    pub agent_name: &'a str,
    pub attended: bool,
    pub action: &'a str,
    pub cap: AgentCap,
    pub cap_source: &'a str,
    pub trace_id: &'a TraceId,
}

impl DocumentRepository {
    /// Every override on `document` and on `folders`; pass a nil document
    /// id for a folder target.
    pub async fn cap_overrides(
        &self,
        document: &DocumentId,
        folders: &[DocumentFolderId],
    ) -> RepoResult<Vec<CapOverride>> {
        let mut overrides = Vec::new();
        let on_document = sqlx::query_scalar!(
            r#"SELECT agent_cap FROM documents WHERE id = $1::text::uuid"#,
            document.as_str(),
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .flatten();
        if let Some(cap) = on_document {
            overrides.push(CapOverride::Document(
                document.clone(),
                parse("documents.agent_cap", &cap)?,
            ));
        }
        if folders.is_empty() {
            return Ok(overrides);
        }
        let folder_ids: Vec<String> = folders.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT id::text AS "id!: DocumentFolderId", agent_cap AS "agent_cap!"
               FROM document_folders
               WHERE id = ANY($1::text[]::uuid[]) AND agent_cap IS NOT NULL"#,
            &folder_ids,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        for row in rows {
            overrides.push(CapOverride::Folder(
                row.id,
                parse("document_folders.agent_cap", &row.agent_cap)?,
            ));
        }
        Ok(overrides)
    }

    pub async fn set_document_agent_cap(
        &self,
        document: &DocumentId,
        cap: Option<AgentCap>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            r#"UPDATE documents SET agent_cap = $2, updated_at = now()
               WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
            document.as_str(),
            cap.map(AgentCap::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_folder_agent_cap(
        &self,
        folder: &DocumentFolderId,
        cap: Option<AgentCap>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            r#"UPDATE document_folders SET agent_cap = $2, updated_at = now()
               WHERE id = $1::text::uuid AND deleted_at IS NULL"#,
            folder.as_str(),
            cap.map(AgentCap::as_str),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn insert_agent_read(&self, record: AgentReadRecord<'_>) -> RepoResult<()> {
        sqlx::query!(
            r#"INSERT INTO document_agent_reads
                   (document_id, user_id, agent_name, attended, action, effective_cap,
                    cap_source, trace_id)
               VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7, $8)"#,
            record.document_id.as_str(),
            record.user_id.as_str(),
            record.agent_name,
            record.attended,
            record.action,
            record.cap.as_str(),
            record.cap_source,
            record.trace_id.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(())
    }
}
