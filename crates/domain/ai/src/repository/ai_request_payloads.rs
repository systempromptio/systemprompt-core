use crate::error::RepositoryError;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiRequestPayloadRepository {
    write_pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct AiRequestPayload {
    pub ai_request_id: AiRequestId,
    pub request_body: Option<Value>,
    pub response_body: Option<Value>,
    pub request_excerpt: Option<String>,
    pub response_excerpt: Option<String>,
    pub request_truncated: bool,
    pub response_truncated: bool,
    pub request_bytes: Option<i32>,
    pub response_bytes: Option<i32>,
}

impl AiRequestPayloadRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }

    pub async fn upsert_request(
        &self,
        ai_request_id: &AiRequestId,
        body: Option<&Value>,
        excerpt: Option<&str>,
        truncated: bool,
        bytes: Option<i32>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, request_body, request_excerpt,
                request_truncated, request_bytes, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET request_body = EXCLUDED.request_body,
                request_excerpt = EXCLUDED.request_excerpt,
                request_truncated = EXCLUDED.request_truncated,
                request_bytes = EXCLUDED.request_bytes,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            body,
            excerpt,
            truncated,
            bytes
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn upsert_response(
        &self,
        ai_request_id: &AiRequestId,
        body: Option<&Value>,
        excerpt: Option<&str>,
        truncated: bool,
        bytes: Option<i32>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, response_body, response_excerpt,
                response_truncated, response_bytes, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET response_body = EXCLUDED.response_body,
                response_excerpt = EXCLUDED.response_excerpt,
                response_truncated = EXCLUDED.response_truncated,
                response_bytes = EXCLUDED.response_bytes,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            body,
            excerpt,
            truncated,
            bytes
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }
}
