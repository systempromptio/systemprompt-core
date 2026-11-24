//! The sticky `documents.agent_content` provenance flag: set inside the
//! transaction that stores agent-written content, never cleared, and read
//! from the primary so a page an agent has just written is never served
//! under the wider tier because a replica lags. The read also answers true
//! for a document whose agent evidence predates the column (an agent
//! creator, revision or proposal), since migration 044 adds it without a
//! backfill.
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
            r#"SELECT (d.agent_content
                       OR d.created_by_agent IS NOT NULL
                       OR EXISTS (SELECT 1 FROM document_revisions r
                                  WHERE r.document_id = d.id AND r.actor_kind = 'agent')
                       OR EXISTS (SELECT 1 FROM document_proposals p
                                  WHERE p.document_id = d.id AND p.actor_kind = 'agent'))
                      AS "flagged!"
               FROM documents d WHERE d.id = $1::text::uuid"#,
            document as _,
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
            document as _,
            authored,
            settles as _,
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}
