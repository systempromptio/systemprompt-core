//! Maps a terminal receipt onto the domain settlement API and the evaluation
//! reservation it may be bound to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_ai::repository::UpsertPayloadParams;
use systemprompt_ai::repository::ai_requests::{
    SettleCompletion, SettledToolCall, SettlementOutcome, SettlementUsage,
};

use super::{Receipt, Settlement};

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
        let [
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            reasoning_tokens,
            tokens_used,
        ] = completion.usage;
        SettlementOutcome::Completed(SettleCompletion {
            usage: SettlementUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                reasoning_tokens,
                tokens_used,
            },
            cost_microdollars: completion.cost,
            latency_ms: completion.latency,
            upstream_latency_ms: completion.upstream_latency,
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
        SettlementOutcome::Failed { error }
    } else {
        anyhow::bail!("A pending receipt has nothing to settle");
    };
    settlement
        .requests
        .settle(&receipt.request_id, &receipt.user_id, outcome)
        .await?;
    settlement
        .evaluations
        .settle_recorded(&receipt.user_id, &receipt.request_id)
        .await?;
    Ok(())
}
