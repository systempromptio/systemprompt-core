//! The sticky `documents.agent_content` provenance flag: set inside the
//! transaction that stores agent-written content, never cleared, and read
//! from the primary so a page an agent has just written is never served
//! under the wider tier because a replica lags.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{DocumentId, DocumentProposalId};

use super::DocumentRepository;
use super::rows::RepoResult;

impl DocumentRepository {
    pub async fn agent_content(&self, document: &DocumentId) -> RepoResult<bool> {
        let flagged = sqlx::query_scalar!(
            r#"SELECT agent_content FROM documents WHERE id = $1::text::uuid"#,
            document.as_str(),
        )
        .fetch_optional(self.pools.authz())
        .await?;
        Ok(flagged.unwrap_or(false))
    }

    pub async fn mark_agent_content_in(
        conn: &mut PgConnection,
        document: &DocumentId,
        authored: bool,
        settles: Option<&DocumentProposalId>,
    ) -> RepoResult<()> {
        sqlx::query!(
            r#"UPDATE documents SET agent_content = true
               WHERE id = $1::text::uuid
                 AND NOT agent_content
                 AND ($2::bool OR EXISTS (
                     SELECT 1 FROM document_proposals p
                     WHERE p.id = $3::text::uuid AND p.actor_kind = 'agent'
                 ))"#,
            document.as_str(),
            authored,
            settles.map(DocumentProposalId::as_str),
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}
