//! Maps a terminal receipt onto the domain settlement API.
//!
//! Bumping the session counters a settled completion consumed is
//! fire-and-forget: the request is already settled, so a counter failure is
//! logged, never propagated. System traffic has no session to account against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_ai::repository::UpsertPayloadParams;
use systemprompt_ai::repository::ai_requests::{
    SettleCompletion, SettledFailure, SettledToolCall, SettlementOutcome, SettlementUsage,
};

use super::{PartialUsage, Receipt, Settlement};

pub(super) async fn settle(settlement: &Settlement, receipt: &Receipt) -> Result<()> {
    if let Some(error) = &receipt.accounting_failure {
        anyhow::ensure!(
            receipt.completion.is_none() && receipt.failure.is_none(),
            "Accounting failure cannot replace a provider receipt"
        );
        settlement
            .requests
            .mark_accounting_failed(&receipt.request_id, &receipt.user_id, error)
            .await?;
        return Ok(());
    }
    let tools: Vec<SettledToolCall>;
    let mut session_usage: Option<(i32, i64)> = None;
    let outcome = if let Some(completion) = &receipt.completion {
        tools = completion
            .tools
            .iter()
            .map(|tool| SettledToolCall {
                id: tool.id.clone(),
                name: tool.name.clone(),
                input: tool.input.clone(),
            })
            .collect();
        let usage = settlement_usage(completion.usage);
        session_usage = Some((
            i32::try_from(usage.tokens_used).unwrap_or(i32::MAX),
            completion.cost,
        ));
        SettlementOutcome::Completed(SettleCompletion {
            usage,
            cost_microdollars: completion.cost,
            latency_ms: completion.latency,
            upstream_latency_ms: completion.upstream_latency,
            finish_reason: completion.finish_reason.as_deref(),
            payload: UpsertPayloadParams {
                body: completion.payload.json.as_ref(),
                excerpt: completion.payload.excerpt.as_deref(),
                truncated: completion.payload.truncated,
                bytes: Some(completion.payload.byte_len),
                sha256: Some(&completion.payload.sha256),
            },
            assistant_text: completion.assistant.as_deref(),
            tool_calls: &tools,
        })
    } else if let Some(error) = &receipt.failure {
        failed_outcome(error, receipt.partial.as_ref())
    } else {
        anyhow::bail!("A pending receipt has nothing to settle");
    };
    settlement
        .requests
        .settle(&receipt.request_id, &receipt.user_id, outcome)
        .await?;
    if let Some((tokens, cost)) = session_usage {
        increment_session_usage(settlement, receipt, tokens, cost).await;
    }
    Ok(())
}

const fn settlement_usage(raw: [u32; 6]) -> SettlementUsage {
    let [
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        reasoning_tokens,
        tokens_used,
    ] = raw;
    SettlementUsage {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        reasoning_tokens,
        tokens_used,
    }
}

// Why: a provider bills what it streamed before the stream broke, so a
// failure carrying partial usage settles with it; absent usage leaves the
// stored columns untouched rather than writing zeroes over them.
fn failed_outcome<'a>(error: &'a str, partial: Option<&'a PartialUsage>) -> SettlementOutcome<'a> {
    SettlementOutcome::Failed(SettledFailure {
        error,
        usage: partial.map(|p| settlement_usage(p.usage)),
        cost_microdollars: partial.map_or(0, |p| p.cost),
        latency_ms: partial.map(|p| p.latency),
        upstream_latency_ms: partial.and_then(|p| p.upstream_latency),
    })
}

async fn increment_session_usage(
    settlement: &Settlement,
    receipt: &Receipt,
    tokens: i32,
    cost_microdollars: i64,
) {
    if receipt.user_id.as_str() == "system" {
        return;
    }
    let (Some(sessions), Some(session_id)) = (&settlement.sessions, &receipt.session_id) else {
        return;
    };
    if let Err(e) = sessions
        .increment_ai_usage(session_id, tokens, cost_microdollars)
        .await
    {
        tracing::warn!(
            error = %e,
            session_id = %session_id,
            ai_request_id = %receipt.request_id,
            "increment_ai_usage failed"
        );
    }
}
