//! Repository for `ai_gateway_thought_signatures` rows: Gemini thought
//! signatures keyed by gateway conversation and `tool_use` id.
//!
//! Reads go to the write pool: a signature captured on one replica must be
//! visible to the replica serving the very next turn, so replica lag is not
//! acceptable here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_database::DbPool;
use systemprompt_identifiers::GatewayConversationId;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiThoughtSignatureRepository {
    write_pool: Arc<PgPool>,
}

impl AiThoughtSignatureRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }

    pub async fn upsert(
        &self,
        conversation: &GatewayConversationId,
        tool_use_id: &str,
        signature: &str,
        ttl: Duration,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_gateway_thought_signatures
                (conversation_id, tool_use_id, signature, expires_at)
            VALUES ($1, $2, $3, NOW() + make_interval(secs => $4))
            ON CONFLICT (conversation_id, tool_use_id) DO UPDATE SET
                signature = EXCLUDED.signature,
                expires_at = EXCLUDED.expires_at
            "#,
            conversation.as_str(),
            tool_use_id,
            signature,
            ttl.as_secs_f64(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn find(
        &self,
        conversation: &GatewayConversationId,
        tool_use_id: &str,
        ttl: Duration,
    ) -> Result<Option<String>, RepositoryError> {
        let row = sqlx::query_scalar!(
            r#"
            UPDATE ai_gateway_thought_signatures
            SET expires_at = NOW() + make_interval(secs => $3)
            WHERE conversation_id = $1
              AND tool_use_id = $2
              AND expires_at > NOW()
            RETURNING signature
            "#,
            conversation.as_str(),
            tool_use_id,
            ttl.as_secs_f64(),
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(row)
    }

    pub async fn cleanup_expired(&self) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query!(r#"DELETE FROM ai_gateway_thought_signatures WHERE expires_at <= NOW()"#)
                .execute(&*self.write_pool)
                .await?;
        Ok(result.rows_affected())
    }
}
