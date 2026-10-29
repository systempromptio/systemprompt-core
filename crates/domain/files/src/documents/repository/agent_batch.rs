//! The agent facts of many documents in two statements: each document's
//! own override and creating agent, and each folder's nearest-first chain
//! with the overrides set on it, for an agent's listing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, DocumentId};

use super::DocumentRepository;
use super::agent_caps::{AgentFacts, CapOverride};
use super::rows::{RepoResult, parse};
use crate::documents::agents::AgentCap;

/// A folder's chain, nearest-first from the folder itself, and every
/// override on that chain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderChain {
    pub chain: Vec<DocumentFolderId>,
    pub overrides: Vec<CapOverride>,
}

impl DocumentRepository {
    pub async fn agent_facts_of(
        &self,
        documents: &[DocumentId],
    ) -> RepoResult<HashMap<DocumentId, AgentFacts>> {
        let ids: Vec<String> = documents.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT id::text AS "id!: DocumentId", agent_cap, created_by_agent
               FROM documents
               WHERE id = ANY($1::text[]::uuid[])
                 AND (agent_cap IS NOT NULL OR created_by_agent IS NOT NULL)"#,
            &ids,
        )
        .fetch_all(self.pools.authz())
        .await?;
        let mut facts = HashMap::with_capacity(rows.len());
        for row in rows {
            let mut found = AgentFacts {
                overrides: Vec::new(),
                created_by_agent: row.created_by_agent,
            };
            if let Some(cap) = row.agent_cap {
                let cap: AgentCap = parse("documents.agent_cap", &cap)?;
                found
                    .overrides
                    .push(CapOverride::Document(row.id.clone(), cap));
            }
            facts.insert(row.id, found);
        }
        Ok(facts)
    }

    pub async fn folder_chains(
        &self,
        folders: &[DocumentFolderId],
    ) -> RepoResult<HashMap<DocumentFolderId, FolderChain>> {
        let ids: Vec<String> = folders.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT f.id::text AS "id!: DocumentFolderId",
                      f.ancestor_ids::text[] AS "ancestor_ids!: Vec<DocumentFolderId>",
                      ARRAY(SELECT a.id::text || ':' || a.agent_cap
                            FROM document_folders a
                            WHERE (a.id = f.id OR a.id = ANY(f.ancestor_ids))
                              AND a.agent_cap IS NOT NULL) AS "caps!: Vec<String>"
               FROM document_folders f
               WHERE f.id = ANY($1::text[]::uuid[])"#,
            &ids,
        )
        .fetch_all(self.pools.authz())
        .await?;
        let mut chains = HashMap::with_capacity(rows.len());
        for row in rows {
            let mut chain = vec![row.id.clone()];
            chain.extend(row.ancestor_ids.into_iter().rev());
            let mut overrides = Vec::new();
            for entry in &row.caps {
                let Some((folder, cap)) = entry.split_once(':') else {
                    continue;
                };
                let cap: AgentCap = parse("document_folders.agent_cap", cap)?;
                overrides.push(CapOverride::Folder(
                    DocumentFolderId::from_uuid(parse("document_folders.id", folder)?),
                    cap,
                ));
            }
            chains.insert(row.id, FolderChain { chain, overrides });
        }
        Ok(chains)
    }
}
