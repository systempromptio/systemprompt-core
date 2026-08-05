//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, EvalCaseId, UserId};

use crate::error::Result;
use crate::models::{EvalCase, NewCaseParams};

#[derive(Debug, Clone)]
pub struct EvalCaseRepository {
    pool: Arc<PgPool>,
}

impl EvalCaseRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?,
        })
    }

    pub async fn create(&self, params: &NewCaseParams) -> Result<EvalCaseId> {
        let id = EvalCaseId::generate();
        let prompt_body = serde_json::to_value(&params.prompt)?;
        let canonical_messages = serde_json::to_value(&params.prompt.messages)?;
        sqlx::query!(
            r#"
            INSERT INTO eval_cases (
                id, name, prompt_body, source_ai_request_id, expectation,
                tags, created_by, canonical_messages, system_prompt,
                offered_tools, provider, model, prepared_body_sha256
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            id.as_str(),
            params.name,
            prompt_body,
            params
                .source_ai_request_id
                .as_ref()
                .map(AiRequestId::as_str),
            params.expectation.as_deref(),
            &params.tags,
            params.created_by.as_str(),
            canonical_messages,
            params.prompt.system_prompt.as_deref(),
            params.prompt.offered_tools.as_ref(),
            params.prompt.provider,
            params.prompt.model,
            params.prepared_body_sha256.as_deref()
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(id)
    }

    pub async fn list_enabled(&self) -> Result<Vec<EvalCase>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, name, prompt_body, source_ai_request_id, expectation,
                   tags, enabled, created_by, created_at, repair_hint,
                   canonical_messages, system_prompt, offered_tools,
                   provider, model, prepared_body_sha256
            FROM eval_cases
            WHERE enabled = TRUE
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| EvalCase {
                id: EvalCaseId::new(row.id),
                name: row.name,
                prompt_body: row.prompt_body,
                source_ai_request_id: row.source_ai_request_id.map(AiRequestId::new),
                expectation: row.expectation,
                tags: row.tags,
                enabled: row.enabled,
                created_by: UserId::new(row.created_by),
                created_at: row.created_at,
                repair_hint: row.repair_hint,
                canonical_messages: row.canonical_messages,
                system_prompt: row.system_prompt,
                offered_tools: row.offered_tools,
                provider: row.provider,
                model: row.model,
                prepared_body_sha256: row.prepared_body_sha256,
            })
            .collect())
    }

    pub async fn set_enabled(&self, id: &EvalCaseId, enabled: bool) -> Result<()> {
        sqlx::query!(
            "UPDATE eval_cases SET enabled = $2 WHERE id = $1",
            id.as_str(),
            enabled
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn set_repair_hint(&self, id: &EvalCaseId, repair_hint: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE eval_cases SET repair_hint = $2 WHERE id = $1",
            id.as_str(),
            repair_hint
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
