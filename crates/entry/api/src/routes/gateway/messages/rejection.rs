use axum::http::StatusCode;
use bytes::Bytes;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, UpsertPayloadParams,
};
use systemprompt_identifiers::{AiRequestId, UserId};
use systemprompt_runtime::AppContext;

use super::extract::RejectionPartial;

pub(super) async fn persist_rejection(
    ctx: &AppContext,
    ai_request_id: &AiRequestId,
    partial: &RejectionPartial,
    status: StatusCode,
    message: &str,
) {
    let repo = match AiRequestRepository::new(ctx.db_pool()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "rejection audit: repo unavailable");
            return;
        },
    };

    let record = build_rejection_record(ai_request_id, partial);
    write_rejection_record(&repo, ai_request_id, &record, status, message).await;

    if let Some(body) = partial.body.as_ref() {
        write_rejection_payload(ctx, ai_request_id, body).await;
    }
}

fn build_rejection_record(
    ai_request_id: &AiRequestId,
    partial: &RejectionPartial,
) -> AiRequestRecord {
    let user_id = partial
        .user_id
        .clone()
        .unwrap_or_else(|| UserId::new("anonymous"));
    let provider = partial
        .provider
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let model = partial
        .model
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let mut builder = AiRequestRecord::builder(ai_request_id.clone(), user_id)
        .provider(provider)
        .model(model)
        .streaming(partial.is_streaming);
    if let Some(t) = &partial.tenant_id {
        builder = builder.tenant_id(t.clone());
    }
    if let Some(s) = &partial.session_id {
        builder = builder.session_id(s.clone());
    }
    if let Some(t) = &partial.trace_id {
        builder = builder.trace_id(t.clone());
    }
    if let Some(mt) = partial.max_tokens {
        builder = builder.max_tokens(mt);
    }
    builder.build().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "rejection audit: build failed");
        AiRequestRecord::minimal_fallback(ai_request_id.as_str().to_string())
    })
}

async fn write_rejection_record(
    repo: &AiRequestRepository,
    ai_request_id: &AiRequestId,
    record: &AiRequestRecord,
    status: StatusCode,
    message: &str,
) {
    if let Err(e) = repo.insert_with_id(ai_request_id, record).await {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: insert failed");
        return;
    }
    let err_msg = format!("HTTP {}: {message}", status.as_u16());
    if let Err(e) = repo.update_error(ai_request_id, &err_msg).await {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: update_error failed");
    }
}

async fn write_rejection_payload(ctx: &AppContext, ai_request_id: &AiRequestId, body: &Bytes) {
    let payloads = match AiRequestPayloadRepository::new(ctx.db_pool()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "rejection audit: payload repo unavailable");
            return;
        },
    };
    let bytes_len = body.len().min(i32::MAX as usize) as i32;
    let body_json = serde_json::from_slice::<serde_json::Value>(body).ok();
    let excerpt = if body_json.is_none() {
        Some(String::from_utf8_lossy(body).to_string())
    } else {
        None
    };
    if let Err(e) = payloads
        .upsert_request(
            ai_request_id,
            UpsertPayloadParams {
                body: body_json.as_ref(),
                excerpt: excerpt.as_deref(),
                truncated: false,
                bytes: Some(bytes_len),
            },
        )
        .await
    {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: payload insert failed");
    }
}
