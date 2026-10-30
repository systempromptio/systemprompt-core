//! [`ExtractionRepository`]: one row per extraction call, keyed so a replay
//! of the same event under the same pipeline version never records (or
//! bills) the call twice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{AiRequestId, BrainExtractionId};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::NewExtraction;

#[derive(Debug, Clone)]
pub struct ExtractionRepository {
    pools: Pools,
}

impl ExtractionRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        conn: &mut PgConnection,
        extraction: &NewExtraction,
    ) -> Result<bool, RepositoryError> {
        let record = &extraction.record;
        let id = BrainExtractionId::generate();
        let result = sqlx::query!(
            "INSERT INTO brain_extractions
                 (id, outbox_id, pipeline_version, extractor, kind, model, prompt_version,
                  raw_output, input_tokens, output_tokens, candidates, ai_request_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (outbox_id, pipeline_version, extractor, kind) DO NOTHING",
            id.as_str(),
            extraction.outbox_id.as_str(),
            extraction.pipeline_version.as_str(),
            record.extractor,
            extraction.kind.as_str(),
            record.model,
            record.prompt_version,
            record.raw_output,
            saturating_i32(record.input_tokens),
            saturating_i32(record.output_tokens),
            saturating_i32(extraction.candidates),
            record.ai_request_id.as_ref().map(AiRequestId::as_str),
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn tokens_since(
        executor: impl PgExecutor<'_>,
        since: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(input_tokens + output_tokens), 0)::bigint AS "tokens!"
               FROM brain_extractions WHERE created_at >= $1"#,
            since,
        )
        .fetch_one(executor)
        .await?)
    }

    pub async fn tokens_used_since(&self, since: DateTime<Utc>) -> Result<i64, RepositoryError> {
        Self::tokens_since(&*self.pools.read, since).await
    }
}

pub(super) fn saturating_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
