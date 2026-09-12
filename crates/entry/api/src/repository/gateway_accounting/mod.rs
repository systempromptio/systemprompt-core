//! Transactional, replayable gateway completion settlement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
pub(crate) mod types;

use anyhow::{Result, ensure};
use sqlx::{PgPool, Postgres, Transaction};
use types::{Completion, Receipt};

pub(crate) async fn settle(pool: &PgPool, receipt: &Receipt) -> Result<()> {
    let mut tx = pool.begin().await?;
    let owner = sqlx::query_scalar!(
        "SELECT user_id FROM ai_requests WHERE id=$1 FOR UPDATE",
        receipt.request_id.as_str()
    )
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        owner == receipt.user_id.as_str(),
        "Journal owner conflicts with request owner"
    );
    if let Some(completion) = &receipt.completion {
        complete(&mut tx, receipt, completion).await?;
    } else if let Some(error) = &receipt.failure {
        sqlx::query!(
            "UPDATE ai_requests SET status='failed',error_message=$2,completed_at=coalesce(completed_at,now()),updated_at=now() WHERE id=$1 AND status<>'completed'",
            receipt.request_id.as_str(), error
        ).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    systemprompt_evaluation::repository::experiments::GatewayEvaluationRepository::new(
        pool.clone(),
    )
    .settle_recorded(&receipt.user_id, &receipt.request_id)
    .await?;
    Ok(())
}

async fn complete(
    tx: &mut Transaction<'_, Postgres>,
    receipt: &Receipt,
    completion: &Completion,
) -> Result<()> {
    let previous = sqlx::query_scalar!(
        "SELECT response_body_sha256 FROM ai_request_payloads WHERE ai_request_id=$1",
        receipt.request_id.as_str()
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    ensure!(
        previous
            .as_ref()
            .is_none_or(|value| value == &completion.payload.sha256),
        "Conflicting terminal response"
    );
    if previous.is_none() {
        insert_messages(tx, receipt, completion).await?;
    }
    let payload = &completion.payload;
    sqlx::query!(
        "INSERT INTO ai_request_payloads(ai_request_id,response_body,response_excerpt,response_truncated,response_bytes,response_body_sha256) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(ai_request_id) DO UPDATE SET response_body=EXCLUDED.response_body,response_excerpt=EXCLUDED.response_excerpt,response_truncated=EXCLUDED.response_truncated,response_bytes=EXCLUDED.response_bytes,response_body_sha256=EXCLUDED.response_body_sha256,updated_at=now()",
        receipt.request_id.as_str(), payload.json.as_ref(), payload.excerpt.as_deref(),
        payload.truncated, payload.byte_len, payload.sha256
    ).execute(&mut **tx).await?;
    let [input, output, cache_read, cache_creation, reasoning, total] = completion.usage;
    sqlx::query!(
        "UPDATE ai_requests SET input_tokens=$2,output_tokens=$3,cache_read_tokens=$4,cache_creation_tokens=$5,reasoning_tokens=$6,tokens_used=$7,cost_microdollars=$8,latency_ms=$9,upstream_latency_ms=$10,cache_hit=$11,status='completed',completed_at=coalesce(completed_at,now()),updated_at=now() WHERE id=$1",
        receipt.request_id.as_str(), i32::try_from(input)?, i32::try_from(output)?,
        i32::try_from(cache_read)?, i32::try_from(cache_creation)?, i32::try_from(reasoning)?,
        i32::try_from(total)?, completion.cost, completion.latency, completion.upstream_latency, cache_read > 0
    ).execute(&mut **tx).await?;
    Ok(())
}

async fn insert_messages(
    tx: &mut Transaction<'_, Postgres>,
    receipt: &Receipt,
    completion: &Completion,
) -> Result<()> {
    for (index, (id, name, input)) in completion.tools.iter().enumerate() {
        sqlx::query!(
            "INSERT INTO ai_request_tool_calls(request_id,ai_tool_call_id,tool_name,tool_input,sequence_number) VALUES($1,$2,$3,$4,$5)",
            receipt.request_id.as_str(), id, name, input, i32::try_from(index + 1)?
        ).execute(&mut **tx).await?;
    }
    if let Some(text) = &completion.assistant {
        sqlx::query!(
            "INSERT INTO ai_request_messages(request_id,role,content,sequence_number) SELECT $1::varchar,'assistant',$2,coalesce(max(sequence_number),-1)+1 FROM ai_request_messages WHERE request_id=$1::varchar",
            receipt.request_id.as_str(), text
        ).execute(&mut **tx).await?;
    }
    Ok(())
}
