//! The recall stage: asks the attached [`MemoryRecall`] engine for the
//! organisation's memory under the profile's deadline and appends the
//! rendered `<team_memory>` block to the request's system prompt.
//!
//! Every outcome, injected or skipped, is written to the audit row's
//! `memory_recall` descriptor; recall never fails the request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderValue, Response};
use systemprompt_extension::brain::{
    MemoryRecall, RecallConversation, RecallOutcome, RecallPrincipal, RecallReceipt, RecallRequest,
    ReuseHint, SkipReason,
};
use systemprompt_identifiers::headers::MEMORY_IDS;
use systemprompt_manifest::profile::BrainRecallProfile;
use systemprompt_models::origin::ClientKind;
use systemprompt_wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role, SystemBlock, is_new_user_turn,
};

use super::super::super::audit::{GatewayAudit, GatewayRequestContext};

pub(in crate::service) struct RecallStage<'a> {
    pub engine: Option<&'a Arc<dyn MemoryRecall>>,
    pub ctx: &'a GatewayRequestContext,
    pub profile: &'a BrainRecallProfile,
}

#[derive(Debug, Default)]
pub(in crate::service) struct RecallApplied {
    pub injected: bool,
    pub ids_header: Option<String>,
    pub receipt: Option<RecallReceipt>,
}

pub(super) async fn apply_team_memory(
    stage: RecallStage<'_>,
    request: &mut CanonicalRequest,
    audit: &GatewayAudit,
) -> RecallApplied {
    let RecallStage {
        engine,
        ctx,
        profile,
    } = stage;
    let Some(engine) = engine else {
        return skipped(SkipReason::Disabled, audit).await;
    };
    if ctx.memory.is_off() {
        return skipped(SkipReason::HeaderOff, audit).await;
    }
    if ctx.origin.client == ClientKind::Internal || !profile.client_enabled(ctx.origin.client) {
        return skipped(SkipReason::ClientNotEnabled, audit).await;
    }

    let recall = recall_request(ctx, profile, &request.messages);
    let outcome = match tokio::time::timeout(recall.deadline, engine.recall(&recall)).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            tracing::warn!(
                ai_request_id = %ctx.ai_request_id,
                engine = engine.name(),
                reason = SkipReason::Unavailable.as_str(),
                error = %e,
                "memory recall skipped"
            );
            RecallOutcome::Skipped {
                reason: SkipReason::Unavailable,
            }
        },
        Err(_) => {
            tracing::warn!(
                ai_request_id = %ctx.ai_request_id,
                engine = engine.name(),
                reason = SkipReason::Deadline.as_str(),
                deadline_ms = profile.deadline_ms,
                "memory recall skipped"
            );
            RecallOutcome::Skipped {
                reason: SkipReason::Deadline,
            }
        },
    };

    audit.set_memory_recall(&outcome.descriptor()).await;
    let (injected, ids_header) = match &outcome {
        RecallOutcome::Injected(block) => {
            request.system.push(SystemBlock::text(block.render()));
            (true, Some(block.ids()).filter(|ids| !ids.is_empty()))
        },
        RecallOutcome::Skipped { .. } => (false, None),
    };
    RecallApplied {
        injected,
        ids_header,
        receipt: Some(RecallReceipt::new(
            &recall,
            ctx.ai_request_id.clone(),
            outcome,
        )),
    }
}

async fn skipped(reason: SkipReason, audit: &GatewayAudit) -> RecallApplied {
    audit.set_memory_recall(&reason.descriptor()).await;
    RecallApplied::default()
}

fn recall_request(
    ctx: &GatewayRequestContext,
    profile: &BrainRecallProfile,
    messages: &[CanonicalMessage],
) -> RecallRequest {
    let principal = RecallPrincipal {
        user_id: ctx.user_id.clone(),
        client: ctx.origin.client,
        actor: ctx.actor.clone(),
        project: None,
        client_id: ctx.client_id.clone(),
    };
    let conversation = RecallConversation {
        ai_request_id: Some(ctx.ai_request_id.clone()),
        gateway_conversation_id: ctx.gateway_conversation_id.clone(),
        session_id: ctx.session_id.clone(),
        context_id: Some(ctx.context_id.clone()),
    };
    let reuse = if is_new_user_turn(messages) {
        ReuseHint::NewTurn
    } else {
        ReuseHint::SameTurn
    };
    RecallRequest::new(principal, conversation)
        .with_subject_hints(&last_user_text(messages))
        .with_budget_tokens(profile.budget_tokens)
        .with_deadline(Duration::from_millis(profile.deadline_ms))
        .with_reuse(reuse)
}

fn last_user_text(messages: &[CanonicalMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| {
            message
                .content
                .iter()
                .filter_map(|part| match part {
                    CanonicalContent::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub(in crate::service) fn record_recall(
    audit: &GatewayAudit,
    engine: Option<&Arc<dyn MemoryRecall>>,
    memory: &RecallApplied,
) {
    let (Some(engine), Some(receipt)) = (engine, memory.receipt.clone()) else {
        return;
    };
    let engine = Arc::clone(engine);
    audit
        .background()
        .spawn("gateway_memory_recall_record", async move {
            if let Err(e) = engine.record(&receipt).await {
                tracing::warn!(
                    ai_request_id = %receipt.ai_request_id,
                    engine = engine.name(),
                    error = %e,
                    "memory recall receipt not recorded"
                );
            }
        });
}

pub(in crate::service) fn attach_memory_ids(response: &mut Response<Body>, ids: Option<&str>) {
    let Some(value) = ids.and_then(|ids| HeaderValue::from_str(ids).ok()) else {
        return;
    };
    response.headers_mut().insert(MEMORY_IDS, value);
}
