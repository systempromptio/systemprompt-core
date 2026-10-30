//! [`RecallRepository`]: the row of truth for every recall, written after
//! the gateway admits the request, and the stored block a same-turn tool
//! round trip reuses byte for byte.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::{
    AiRequestId, BrainRecallId, ContextId, GatewayConversationId, SessionId,
};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{NewRecall, StoredBlock};

#[derive(Debug, Clone)]
pub struct RecallRepository {
    pools: Pools,
}

impl RecallRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        executor: impl PgExecutor<'_>,
        recall: &NewRecall,
    ) -> Result<BrainRecallId, RepositoryError> {
        let audit = &recall.audit;
        let conversation = &recall.conversation;
        sqlx::query!(
            "INSERT INTO brain_recalls
                 (id, principal_user_id, ai_request_id, gateway_conversation_id, session_id,
                  context_id, client, injected, withheld, tokens, as_of, skipped, rendered_block)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            audit.recall_id.as_str(),
            recall.principal.as_str(),
            conversation.ai_request_id.as_ref().map(AiRequestId::as_str),
            conversation
                .gateway_conversation_id
                .as_ref()
                .map(GatewayConversationId::as_str),
            conversation.session_id.as_ref().map(SessionId::as_str),
            conversation.context_id.as_ref().map(ContextId::as_str),
            recall.client.as_str(),
            serde_json::to_value(&audit.injected)?,
            serde_json::to_value(&audit.withheld)?,
            i32::try_from(audit.tokens).unwrap_or(i32::MAX),
            audit.as_of,
            recall.skipped,
            recall.rendered_block,
        )
        .execute(executor)
        .await?;
        Ok(audit.recall_id.clone())
    }

    pub async fn stored_block(
        executor: impl PgExecutor<'_>,
        conversation: &GatewayConversationId,
    ) -> Result<Option<StoredBlock>, RepositoryError> {
        Ok(sqlx::query_as!(
            StoredBlock,
            r#"SELECT id AS "recall_id: BrainRecallId", rendered_block AS "rendered_block!", as_of
               FROM brain_recalls
               WHERE gateway_conversation_id = $1 AND rendered_block IS NOT NULL
               ORDER BY created_at DESC LIMIT 1"#,
            conversation.as_str(),
        )
        .fetch_optional(executor)
        .await?)
    }

    pub async fn stored_block_for(
        &self,
        conversation: &GatewayConversationId,
    ) -> Result<Option<StoredBlock>, RepositoryError> {
        Self::stored_block(&*self.pools.read, conversation).await
    }

    pub async fn record_detached(
        &self,
        recall: &NewRecall,
    ) -> Result<BrainRecallId, RepositoryError> {
        Self::record(&*self.pools.write, recall).await
    }

    pub async fn purge_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_recalls WHERE id IN (
                 SELECT id FROM brain_recalls WHERE created_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
