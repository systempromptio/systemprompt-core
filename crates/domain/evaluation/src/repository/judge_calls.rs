//! Repository for judge-call audit rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiRequestId, EvalResultId, EvalRubricId, EvalRunId, GatewayConversationId,
};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct EvalJudgeCallRepository {
    pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct JudgeCallRecord<'a> {
    pub conversation_id: &'a GatewayConversationId,
    pub run_id: &'a EvalRunId,
    pub result_id: Option<&'a EvalResultId>,
    pub judge_ai_request_id: Option<&'a AiRequestId>,
    pub rubric_id: Option<&'a EvalRubricId>,
    pub cost_microdollars: i64,
}

impl EvalJudgeCallRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?,
        })
    }

    pub async fn insert(&self, record: &JudgeCallRecord<'_>) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO eval_judge_calls (
                conversation_id, run_id, result_id, judge_ai_request_id,
                rubric_id, cost_microdollars
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (conversation_id) DO UPDATE
            SET result_id = EXCLUDED.result_id,
                judge_ai_request_id = EXCLUDED.judge_ai_request_id,
                cost_microdollars = EXCLUDED.cost_microdollars
            "#,
            record.conversation_id.as_str(),
            record.run_id.as_str(),
            record.result_id.map(EvalResultId::as_str),
            record.judge_ai_request_id.map(AiRequestId::as_str),
            record.rubric_id.map(EvalRubricId::as_str),
            record.cost_microdollars
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
