//! Repository for stored AI request/response payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    pub request_body_sha256: Option<String>,
    pub prepared_body_sha256: Option<String>,
    pub response_body_sha256: Option<String>,
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
        params: UpsertPayloadParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, request_body, request_excerpt,
                request_truncated, request_bytes, request_body_sha256,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET request_body = EXCLUDED.request_body,
                request_excerpt = EXCLUDED.request_excerpt,
                request_truncated = EXCLUDED.request_truncated,
                request_bytes = EXCLUDED.request_bytes,
                request_body_sha256 = EXCLUDED.request_body_sha256,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            params.body,
            params.excerpt,
            params.truncated,
            params.bytes,
            params.sha256
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn upsert_response(
        &self,
        ai_request_id: &AiRequestId,
        params: UpsertPayloadParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, response_body, response_excerpt,
                response_truncated, response_bytes, response_body_sha256,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET response_body = EXCLUDED.response_body,
                response_excerpt = EXCLUDED.response_excerpt,
                response_truncated = EXCLUDED.response_truncated,
                response_bytes = EXCLUDED.response_bytes,
                response_body_sha256 = EXCLUDED.response_body_sha256,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            params.body,
            params.excerpt,
            params.truncated,
            params.bytes,
            params.sha256
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn upsert_offered_tools(
        &self,
        ai_request_id: &AiRequestId,
        offered_tools: &Value,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, offered_tools, created_at, updated_at
            )
            VALUES ($1, $2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET offered_tools = EXCLUDED.offered_tools,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            offered_tools
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn upsert_prepared_sha256(
        &self,
        ai_request_id: &AiRequestId,
        sha256: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_payloads (
                ai_request_id, prepared_body_sha256, created_at, updated_at
            )
            VALUES ($1, $2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET prepared_body_sha256 = EXCLUDED.prepared_body_sha256,
                updated_at = CURRENT_TIMESTAMP
            "#,
            ai_request_id.as_str(),
            sha256
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UpsertPayloadParams<'a> {
    pub body: Option<&'a Value>,
    pub excerpt: Option<&'a str>,
    pub truncated: bool,
    pub bytes: Option<i32>,
    pub sha256: Option<&'a str>,
}
